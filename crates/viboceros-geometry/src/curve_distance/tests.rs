use super::*;
use crate::Tolerance;
fn line(a: [f64; 3], b: [f64; 3]) -> LineSegment {
    LineSegment::try_new(
        Point3::try_from(a).unwrap(),
        Point3::try_from(b).unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap()
}
#[test]
fn crv_deviation_crossing_lines_follow_bisector_correspondence_under_swap_and_reversal() {
    let a = line([-5., 0., 0.], [5., 0., 0.]);
    for degrees in [5_f64, 30., 45., 80., 89., 100., 175.] {
        let angle = degrees.to_radians();
        let b = line(
            [-5. * angle.cos(), -5. * angle.sin(), 0.],
            [5. * angle.cos(), 5. * angle.sin(), 0.],
        );
        let expected = 10. * (degrees.min(180. - degrees).to_radians() / 2.).sin();
        for (a, b) in [
            (a, b),
            (b, a),
            (line(a.end().to_array(), a.start().to_array()), b),
            (a, line(b.end().to_array(), b.start().to_array())),
        ] {
            let r = line_deviation(a, b).unwrap().unwrap();
            assert!(
                (r.extrema.maximum.distance - expected).abs() < 1e-12,
                "{degrees}"
            );
            assert!(r.extrema.minimum.distance < 1e-12, "{degrees}");
            assert!(
                r.extrema
                    .intervals
                    .iter()
                    .all(|p| p[0].abs() < 1e-14 && (p[1] - 1.).abs() < 1e-14)
            );
            for w in [
                &r.extrema.minimum,
                &r.extrema.maximum,
                &r.ends[0],
                &r.ends[1],
            ] {
                assert!(
                    a.evaluate(w.parameters[0])
                        .unwrap()
                        .distance_to(w.points[0])
                        .unwrap()
                        < 1e-12
                );
                assert!(
                    b.evaluate(w.parameters[1])
                        .unwrap()
                        .distance_to(w.points[1])
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
}
#[test]
fn crv_deviation_parallel_line_clipping_keeps_native_domains_and_visit() {
    let a = line([10., 0., 0.], [0., 0., 0.])
        .try_reparameterized(-7. ..=13.)
        .unwrap();
    let b = line([3., 2., 0.], [7., 2., 0.])
        .try_reparameterized(100. ..=108.)
        .unwrap();
    let r = line_deviation(a, b).unwrap().unwrap();
    assert_eq!(r.extrema.intervals, [[0.3, 0.7], [0., 1.]]);
    assert_eq!(r.ends[0].parameters, [-1., 108.]);
    assert!((r.ends[1].parameters[0] - 7.).abs() < 1e-14);
    assert_eq!(r.ends[1].parameters[1], 100.);
    assert_eq!(r.extrema.minimum.distance, 2.);
    assert_eq!(r.extrema.maximum.distance, 2.);
}
#[test]
fn crv_deviation_line_no_overlap_and_skew_correspondence() {
    let a = line([-5., 0., 0.], [5., 0., 0.]);
    for b in [
        line([0., -5., 0.], [0., 5., 0.]),
        line([5., 2., 0.], [10., 2., 0.]),
        line([6., 2., 0.], [10., 2., 0.]),
    ] {
        assert!(line_deviation(a, b).unwrap().is_none());
    }
    let b = line([-3., -4., 7.], [3., 4., 7.]);
    let r = line_deviation(a, b).unwrap().unwrap();
    assert!((r.extrema.minimum.distance - 7.).abs() < 1e-12);
    assert!((r.extrema.maximum.distance - 69_f64.sqrt()).abs() < 1e-12);
}
#[test]
fn crv_deviation_line_extreme_scales_and_translation_do_not_overflow_intermediates() {
    for scale in [1e-150, 1., 1e150] {
        let a = line([-5. * scale, 0., 0.], [5. * scale, 0., 0.]);
        let b = line(
            [-3. * scale, -4. * scale, 2. * scale],
            [3. * scale, 4. * scale, 2. * scale],
        );
        let r = line_deviation(a, b).unwrap().unwrap();
        assert!((r.extrema.minimum.distance / scale - 2.).abs() < 1e-12);
        assert!((r.extrema.maximum.distance / scale - 24_f64.sqrt()).abs() < 1e-12);
    }
    let base = 1e150;
    let step = 1e140;
    let a = line([base, base, base], [base + 10. * step, base, base]);
    let b = line(
        [base + 3. * step, base + 2. * step, base],
        [base + 7. * step, base + 2. * step, base],
    );
    let r = line_deviation(a, b).unwrap().unwrap();
    assert_eq!(r.extrema.minimum.distance, b.start().y() - a.start().y());
}
