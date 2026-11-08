use super::*;
use viboceros_geometry::{BoundingBox3, LineSegment};

#[test]
fn padded_spatial_line_overlaps_preserve_source_parameters_at_support_boundaries() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let mut count = 0;
    for u in [[1., 0., 0.], [0., 1., 0.], [0.6, 0., 0.8], [-0.8, 0., 0.6]] {
        for origin in [[0., 0., 0.], [10., -2., 3.]] {
            for length in [4., 100.] {
                for reverse in [[false, false], [true, false], [false, true], [true, true]] {
                    let point = |x: f64| {
                        Point3::try_from(std::array::from_fn(|i| origin[i] + x * u[i])).unwrap()
                    };
                    let bounds =
                        BoundingBox3::from_points([point(0.), point(2. * length)]).unwrap();
                    let padding = 100. * bounds.min().distance_to(bounds.max()).unwrap();
                    let supports: [NurbsCurve; 2] = std::array::from_fn(|i| {
                        let a = i as f64 * length;
                        let b = a + length;
                        let (a, b) = if reverse[i] { (b, a) } else { (a, b) };
                        let line = LineSegment::try_new(point(a), point(b), tolerance)
                            .unwrap()
                            .try_reparameterized(0. ..=length)
                            .unwrap();
                        let f = padding / line.length().unwrap();
                        LineSegment::try_new(
                            line.point_at(-f).unwrap(),
                            line.point_at(1. + f).unwrap(),
                            tolerance,
                        )
                        .unwrap()
                        .try_reparameterized(-f * length..=(1. + f) * length)
                        .unwrap()
                        .to_nurbs()
                        .unwrap()
                    });
                    let events = curve_intersection_events(&supports[0], &supports[1], tolerance)
                        .unwrap_or_else(|error| panic!("u={u:?} origin={origin:?} length={length} reverse={reverse:?}: {error:?}"));
                    assert_eq!(events.len(), 1);
                    let CurveCurveIntersectionEvent::Overlap(span) = events[0] else {
                        panic!("line overlap")
                    };
                    let mut expected = [length - padding, length + padding];
                    if reverse[0] {
                        expected.reverse();
                    }
                    for (hit, station) in [span.start(), span.end()].into_iter().zip(expected) {
                        assert!(hit.point().distance_to(point(station)).unwrap() < 1e-6);
                        let parameters = [hit.first_parameter(), hit.second_parameter()];
                        for i in 0..2 {
                            let local = station - i as f64 * length;
                            let expected = if reverse[i] { length - local } else { local };
                            assert!((parameters[i] - expected).abs() < 1e-6);
                            assert!(supports[i].domain().contains(&parameters[i]));
                            assert!(
                                supports[i]
                                    .evaluate(parameters[i])
                                    .unwrap()
                                    .distance_to(hit.point())
                                    .unwrap()
                                    < 1e-6
                            );
                        }
                    }
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 64);
    println!("BOUNDED_LINE_OVERLAPS {count}");
}
