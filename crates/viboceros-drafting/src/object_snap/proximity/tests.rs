use super::*;

#[test]
fn projected_segment_distance_retains_tiny_and_overflowing_coordinate_scales() {
    for scale in [1e-300, 1., 1e300] {
        for (a, b, expected) in [
            ([3., -4.], [3., 4.], 3.),
            ([-3., 0.], [3., 0.], 0.),
            ([3., 4.], [6., 8.], 5.),
            ([3., 4.], [3., 4.], 5.),
        ] {
            let result = segment_distance(a.map(|v| v * scale), b.map(|v| v * scale)).unwrap();
            assert!((result / scale - expected).abs() < 1e-12);
            assert_eq!(
                result,
                segment_distance(b.map(|v| v * scale), a.map(|v| v * scale)).unwrap()
            );
        }
    }
    assert_eq!(
        segment_distance([1e308, -1e308], [1e308, 1e308]),
        Some(1e308)
    );
    assert_eq!(
        segment_distance([-Real::MAX, 0.], [Real::MAX, 0.]),
        Some(0.)
    );
    assert_eq!(segment_distance([0., 0.], [0., 0.]), Some(0.));
    assert_eq!(
        segment_distance([-1e300, 1e-300], [1e300, 1e-300]),
        Some(1e-300)
    );
}

#[test]
fn sided_span_proximity_does_not_bridge_discontinuous_curve_branches() {
    use viboceros_geometry::NurbsCurve;
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let curve = NurbsCurve::try_new(
        1,
        vec![p(-4., -1.), p(-4., 1.), p(4., -1.), p(4., 1.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let sampler = curve.parameter_sampler().unwrap();
    let distances: Vec<_> = sampler
        .spans()
        .map(|span| {
            projected_distance(|t| {
                let point = span.evaluate(t).ok()?;
                Some(point.x().hypot(point.y()))
            })
            .unwrap()
        })
        .collect();
    assert_eq!(distances.len(), 2);
    for distance in distances {
        assert!((distance - 4.).abs() < 1e-12);
    }
}
