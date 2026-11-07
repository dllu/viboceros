use viboceros_geometry::{NurbsCurve, Point3};
fn line(spans: usize) -> NurbsCurve {
    let points = (0..=spans)
        .map(|i| Point3::try_new(i as f64, 0., 3.).unwrap())
        .collect();
    let knots = std::iter::once(0.)
        .chain((0..=spans).map(|i| i as f64))
        .chain(std::iter::once(spans as f64))
        .collect();
    NurbsCurve::try_new(1, points, knots).unwrap()
}
#[test]
fn bounded_projection_honors_visibility_endpoints_and_finite_coordinates() {
    let curve = line(2);
    let xy = |p: Point3| Some([p.x(), p.y()]);
    for (target, t) in [([1.25, 4.], 1.25), ([-3., 4.], 0.), ([4., 4.], 2.)] {
        assert!((curve.closest_parameter_in_projection(target, xy).unwrap() - t).abs() < 1e-7);
    }
    assert!(
        curve
            .closest_parameter_in_projection([0., 0.], |_| None)
            .is_none()
    );
    assert!(
        curve
            .closest_parameter_in_projection([0., 0.], |_| Some([f64::NAN, 0.]))
            .is_none()
    );
    assert!(
        curve
            .closest_parameter_in_projection([f64::INFINITY, 0.], xy)
            .is_none()
    );
    let t = curve
        .closest_parameter_in_projection([1.25, 0.], |p| (p.x() > 1.).then_some([p.x(), p.y()]))
        .unwrap();
    assert!((t - 1.25).abs() < 1e-7);
}
#[test]
fn maximum_span_count_is_admitted_and_excess_rejects_before_projection() {
    let t = line(4096)
        .closest_parameter_in_projection([1234.5, 1.], |p| Some([p.x(), p.y()]))
        .unwrap();
    assert!((t - 1234.5).abs() < 1e-7);
    assert!(
        line(4097)
            .closest_parameter_in_projection([0., 0.], |_| panic!("over-budget curve evaluated"))
            .is_none()
    );
}
