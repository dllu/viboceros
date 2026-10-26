use super::*;

#[test]
fn scalar_midpoints_preserve_subnormal_rounding() {
    let unit = Real::from_bits(1);
    for (left, right, expected) in [(1.0, 2.0, 2.0), (-1.0, 2.0, 0.0), (-31.0, -30.0, -30.0)] {
        assert_eq!(finite_midpoint(left * unit, right * unit), expected * unit);
        assert_eq!(finite_midpoint(right * unit, left * unit), expected * unit);
    }
    assert_eq!(finite_midpoint(Real::MAX, Real::MAX), Real::MAX);
    assert_eq!(finite_midpoint(-Real::MAX, Real::MAX), 0.0);
}

#[test]
fn degree_one_curve_interpolates_linearly() {
    let curve = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(2.0, 4.0)],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(curve.evaluate(0.0).unwrap(), point(0.0, 0.0));
    assert_eq!(curve.evaluate(1.0).unwrap(), point(2.0, 4.0));
    assert_eq!(curve.evaluate(0.25).unwrap(), point(0.5, 1.0));
    assert_eq!(
        curve.derivative_at(0.25).unwrap(),
        Vector3::try_new(2.0, 4.0, 0.0).unwrap()
    );
}

#[test]
fn quadratic_bezier_matches_bernstein_result() {
    let curve = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(1.0, 2.0), point(2.0, 0.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    assert_point_near(curve.evaluate(0.5).unwrap(), point(1.0, 1.0));
    assert_eq!(
        curve.derivative_at(0.5).unwrap(),
        Vector3::try_new(2.0, 0.0, 0.0).unwrap()
    );
}

#[test]
fn exact_second_derivative_matches_polynomial_and_rational_curves() {
    let polynomial = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(1.0, 2.0), point(3.0, 1.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let (_, first, second) = polynomial.evaluate_with_second_derivative(0.25).unwrap();
    assert_eq!(first, Vector3::try_new(2.5, 2.5, 0.0).unwrap());
    assert_eq!(second, Vector3::try_new(2.0, -6.0, 0.0).unwrap());

    let middle_weight = 0.5_f64.sqrt();
    let rational = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let (_, _, exact) = rational.evaluate_with_second_derivative(0.5).unwrap();
    let step = 1.0e-5;
    let before = rational.derivative_at(0.5 - step).unwrap();
    let after = rational.derivative_at(0.5 + step).unwrap();
    let finite_difference = Vector3::try_new(
        (after.x() - before.x()) / (2.0 * step),
        (after.y() - before.y()) / (2.0 * step),
        (after.z() - before.z()) / (2.0 * step),
    )
    .unwrap();
    for (actual, expected) in exact
        .to_array()
        .into_iter()
        .zip(finite_difference.to_array())
    {
        assert!((actual - expected).abs() < 2.0e-8);
    }
}

#[test]
fn rational_quadratic_represents_exact_quarter_circle() {
    let middle_weight = 0.5_f64.sqrt();
    let controls = vec![
        WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
        WeightedPoint3::try_new(point(1.0, 1.0), middle_weight).unwrap(),
        WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
    ];
    let curve =
        NurbsCurve::try_new_rational(2, controls, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let coordinate = 0.5_f64.sqrt();
    let midpoint = curve.evaluate(0.5).unwrap();
    assert_point_near(midpoint, point(coordinate, coordinate));
    assert!(Tolerance::DEFAULT.approx_eq(midpoint.x().hypot(midpoint.y()), 1.0));
    let tangent = curve.derivative_at(0.5).unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(tangent.x() + tangent.y(), 0.0));
    let radius = Vector3::try_new(midpoint.x(), midpoint.y(), 0.0).unwrap();
    assert!(Tolerance::DEFAULT.approx_eq(radius.dot(tangent).unwrap(), 0.0));
    assert!(
        Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12)
            .unwrap()
            .approx_eq(
                curve.length(Tolerance::DEFAULT).unwrap(),
                std::f64::consts::FRAC_PI_2
            )
    );
}

#[test]
fn rejects_structural_errors_and_out_of_domain_parameters() {
    let controls = vec![point(0.0, 0.0), point(1.0, 1.0), point(2.0, 0.0)];
    assert!(NurbsCurve::try_new(0, controls.clone(), vec![]).is_err());
    assert!(NurbsCurve::try_new(2, controls.clone(), vec![0.0; 5]).is_err());
    assert!(NurbsCurve::try_new(2, controls.clone(), vec![0.0, 0.0, 0.5, 0.4, 1.0, 1.0]).is_err());

    let curve = NurbsCurve::try_clamped_uniform(2, controls).unwrap();
    assert!(matches!(
        curve.evaluate(-0.1),
        Err(GeometryError::ParameterOutOfDomain { .. })
    ));
    assert!(curve.evaluate(Real::NAN).is_err());
}

#[test]
fn evaluates_across_a_domain_whose_difference_overflows() {
    let curve = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(10.0, 0.0)],
        vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
    )
    .unwrap();
    assert_eq!(curve.parameter_at(0.5).unwrap(), 0.0);
    assert_point_near(curve.evaluate(0.0).unwrap(), point(5.0, 0.0));
    let derivative = curve.derivative_at(0.0).unwrap();
    assert!(derivative.x().is_finite());
    assert!(derivative.x() > 0.0);
}
