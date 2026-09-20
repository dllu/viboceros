use super::*;

fn bezier(controls: &[(Point2, Real)]) -> NurbsCurve2 {
    let degree = controls.len() - 1;
    NurbsCurve2::try_new_rational(
        degree,
        controls
            .iter()
            .map(|&(p, w)| WeightedPoint2::try_new(p, w).unwrap())
            .collect(),
        [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
    )
    .unwrap()
}

#[test]
fn uv_range_loss_recovers_a_silently_erased_coordinate() {
    let small = 2_f64.powi(-700);
    let curve = bezier(&[
        (p(Real::MAX, 0.4), small),
        (p(0., 0.4), 1.),
        (p(0., 0.4), 2_f64.powi(700)),
    ]);
    let actual = curve.evaluate(2_f64.powi(-350)).unwrap();
    assert_eq!(actual.x(), Real::MAX * small);
    assert_eq!(actual.y(), 0.4);
}

#[test]
fn uv_range_loss_retains_the_finite_endpoint_derivative() {
    let a = 1e-200;
    let curve = bezier(&[(p(0., 0.4), 1.), (p(a, 0.4), a)]);
    let (point, derivative) = curve.evaluate_with_derivative(1.).unwrap();
    assert_eq!(point, p(a, 0.4));
    assert_eq!(derivative, [1., 0.]);
}

#[test]
fn uv_range_loss_retains_finite_constant_jets_after_signed_cancellation() {
    let point = p(0.3, 0.4);
    let curve = bezier(&[
        (point, Real::MAX),
        (point, Real::MIN_POSITIVE),
        (point, -Real::MAX),
    ]);
    assert_eq!(curve.evaluate(0.5).unwrap(), point);
    assert_eq!(
        curve.evaluate_with_derivative(0.5).unwrap(),
        (point, [0.; 2])
    );
}

#[test]
fn uv_exact_recovery_handles_a_false_pole_after_lossless_preparation() {
    let point = p(0.3, 0.4);
    let curve = bezier(&[(point, 1.), (point, 2_f64.powi(-100)), (point, -1.)]);
    assert_eq!(
        curve.evaluate_with_derivative(0.5).unwrap(),
        (point, [0.; 2])
    );
}

#[test]
fn uv_exact_recovery_keeps_constant_derivatives_on_subnormal_domains() {
    let point = p(0.3, 0.4);
    let curve = NurbsCurve2::try_new_rational(
        1,
        [1., 2.]
            .map(|w| WeightedPoint2::try_new(point, w).unwrap())
            .to_vec(),
        vec![0., 0., Real::from_bits(1), Real::from_bits(1)],
    )
    .unwrap();
    for t in [0., Real::from_bits(1)] {
        assert_eq!(curve.evaluate_with_derivative(t).unwrap(), (point, [0.; 2]));
    }
}

#[test]
fn uv_exact_recovery_preserves_signed_zero_endpoints_and_real_overflow_errors() {
    let point = p(-0., 0.4);
    let curve = bezier(&[(point, Real::MIN_POSITIVE), (point, Real::MAX)]);
    for t in [0., 1.] {
        assert_eq!(
            curve.evaluate(t).unwrap().to_array().map(Real::to_bits),
            point.to_array().map(Real::to_bits)
        );
        let jet = curve.evaluate_with_derivative(t).unwrap();
        assert_eq!(
            jet.0.to_array().map(Real::to_bits),
            point.to_array().map(Real::to_bits)
        );
        assert_eq!(jet.1, [0.; 2]);
    }
    let curve = bezier(&[(p(0., 0.4), 1.), (p(1., 0.4), Real::from_bits(1))]);
    assert_eq!(curve.evaluate(1.).unwrap(), p(1., 0.4));
    assert!(matches!(
        curve.evaluate_with_derivative(1.),
        Err(GeometryError::NonFinite { .. })
    ));
}

#[test]
fn uv_exact_recovery_preserves_full_order_sides_and_rejects_invalid_parameters() {
    let a = 1e-200;
    let curve = NurbsCurve2::try_new_rational(
        1,
        [
            (p(0., 0.4), 1.),
            (p(a, 0.4), a),
            (p(0.3, 0.7), Real::MAX),
            (p(0.3, 0.7), Real::MIN_POSITIVE),
        ]
        .map(|(p, w)| WeightedPoint2::try_new(p, w).unwrap())
        .to_vec(),
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    let original = curve.clone();
    assert_eq!(
        curve.evaluate_with_derivative_on_side(1., Left).unwrap(),
        (p(a, 0.4), [1., 0.])
    );
    assert_eq!(
        curve.evaluate_with_derivative_on_side(1., Right).unwrap(),
        (p(0.3, 0.7), [0.; 2])
    );
    assert_eq!(curve.evaluate(1.).unwrap(), p(0.3, 0.7));
    for t in [-1., 3., Real::INFINITY, Real::NEG_INFINITY, Real::NAN] {
        assert!(curve.evaluate(t).is_err());
        assert!(curve.evaluate_with_derivative(t).is_err());
    }
    assert_eq!(curve, original);
}

#[test]
fn uv_exact_recovery_does_not_hide_true_poles() {
    let point = p(0.3, 0.4);
    let curve = bezier(&[
        (point, Real::MAX),
        (point, Real::MIN_POSITIVE),
        (point, -Real::MIN_POSITIVE),
        (point, -Real::MAX),
    ]);
    assert_eq!(
        curve.evaluate(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(
        curve.evaluate_with_derivative(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    for t in [0., 0.25, 0.75, 1.] {
        assert_eq!(curve.evaluate_with_derivative(t).unwrap(), (point, [0.; 2]));
    }
}
