use super::*;

#[test]
fn closest_parameter_resolves_an_oscillatory_tangent_projection() {
    // C(t)=(t,t^2), q=(1.998r+2r^3,-0.499). The derivative of squared
    // distance / 2 is 1.998t+2t^3-q.x, strictly increasing on [0,1].
    // Thus r is the unique global minimizer, not just a sample witness.
    let curve = NurbsCurve::try_new(
        2,
        vec![point(0., 0.), point(0.5, 0.), point(1., 1.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let r: Real = 0.001;
    let query = point(1.998 * r + 2. * r.powi(3), -0.499);
    let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-12).unwrap();
    let (refined, _) = curve
        .refine_closest_parameter(query, 0., [0., 1.], tolerance)
        .unwrap();
    let parameter = curve.closest_parameter(query, tolerance).unwrap();
    for actual in [refined, parameter] {
        assert!((actual - r).abs() < 1e-10, "expected {r}, got {actual}");
    }
}

#[test]
fn closest_parameter_retains_first_derivative_fallback_on_extreme_domains() {
    for extent in [1e-200, 1e200] {
        let curve = NurbsCurve::try_new(
            3,
            vec![point(0., 0.), point(0.2, 0.), point(0.8, 0.), point(1., 0.)],
            vec![0., 0., 0., 0., extent, extent, extent, extent],
        )
        .unwrap();
        if extent < 1. {
            assert!(curve.evaluate_with_derivative(extent * 0.3).is_ok());
            assert!(curve.evaluate_with_second_derivative(extent * 0.3).is_err());
        }
        let t = curve
            .closest_parameter(point(0.37, 0.2), Tolerance::DEFAULT)
            .unwrap();
        assert!((curve.evaluate(t).unwrap().x() - 0.37).abs() < 1e-9);
    }
}

#[test]
fn closest_parameter_finds_rational_arc_and_endpoint_minima() {
    let middle_weight = 0.5_f64.sqrt();
    let arc = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let diagonal = 2.0_f64.sqrt();
    let parameter = arc
        .closest_parameter(
            point(diagonal, diagonal),
            Tolerance::try_new(1e-12, 1e-12, 1e-12).unwrap(),
        )
        .unwrap();
    assert!((parameter - 0.5).abs() <= 1.0e-10, "parameter={parameter}");
    assert_point_near(
        arc.evaluate(parameter).unwrap(),
        point(0.5_f64.sqrt(), 0.5_f64.sqrt()),
    );

    let line = NurbsCurve::try_new(
        1,
        vec![point(-2.0, 3.0), point(4.0, 3.0)],
        vec![-5.0, -5.0, 7.0, 7.0],
    )
    .unwrap();
    assert_eq!(
        line.closest_parameter(point(-9.0, 1.0), Tolerance::DEFAULT)
            .unwrap(),
        -5.0
    );
    assert_eq!(
        line.closest_parameter(point(12.0, 5.0), Tolerance::DEFAULT)
            .unwrap(),
        7.0
    );
}

#[test]
fn closest_parameter_resolves_nonuniform_multispan_lobes() {
    let curve = NurbsCurve::try_new_rational(
        3,
        vec![
            WeightedPoint3::try_new(point(-5.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(-4.0, 7.0), 0.8).unwrap(),
            WeightedPoint3::try_new(point(-1.0, -6.0), 1.7).unwrap(),
            WeightedPoint3::try_new(point(1.0, 6.0), 0.6).unwrap(),
            WeightedPoint3::try_new(point(4.0, -7.0), 1.4).unwrap(),
            WeightedPoint3::try_new(point(6.0, 1.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(9.0, 3.0), 0.9).unwrap(),
        ],
        vec![-3.0, -3.0, -3.0, -3.0, -2.4, 0.75, 2.8, 4.0, 4.0, 4.0, 4.0],
    )
    .unwrap();
    let expected_parameter = 1.83;
    let (on_curve, tangent) = curve.evaluate_with_derivative(expected_parameter).unwrap();
    let normal = Vector3::try_new(-tangent.y(), tangent.x(), 0.0)
        .unwrap()
        .normalized(Tolerance::DEFAULT)
        .unwrap();
    let target = on_curve
        .translated(normal.as_vector().scaled(0.025).unwrap())
        .unwrap();
    let actual_parameter = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
    assert!(
        (actual_parameter - expected_parameter).abs() <= 1.0e-8,
        "actual={actual_parameter}, expected={expected_parameter}"
    );
}

#[test]
fn closest_parameter_handles_large_domains_and_translations() {
    let base = 1.0e120;
    let extent = 1.0e114;
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(base - 3.0 * extent, base),
            point(base - extent, base + 4.0 * extent),
            point(base + 2.0 * extent, base - 2.0 * extent),
            point(base + 5.0 * extent, base + extent),
        ],
        vec![
            -1.0e100, -1.0e100, -1.0e100, 2.0e99, 1.0e100, 1.0e100, 1.0e100,
        ],
    )
    .unwrap();
    let expected_parameter = 6.5e99;
    let (on_curve, tangent) = curve.evaluate_with_derivative(expected_parameter).unwrap();
    let normal = Vector3::try_new(-tangent.y(), tangent.x(), 0.0)
        .unwrap()
        .normalized(Tolerance::DEFAULT)
        .unwrap();
    let target = on_curve
        .translated(normal.as_vector().scaled(1.0e112).unwrap())
        .unwrap();
    let actual_parameter = curve.closest_parameter(target, Tolerance::DEFAULT).unwrap();
    assert!(
        ((actual_parameter - expected_parameter) / expected_parameter).abs() <= 1.0e-10,
        "actual={actual_parameter:e}, expected={expected_parameter:e}"
    );
}
