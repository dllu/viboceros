use super::*;

fn bezier(controls: &[(Point3, Real)]) -> NurbsCurve {
    let degree = controls.len() - 1;
    NurbsCurve::try_new_rational(
        degree,
        controls
            .iter()
            .map(|&(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
    )
    .unwrap()
}

#[test]
fn curve_range_loss_recovers_a_large_coordinate_silently_returned_as_zero() {
    let small = 2_f64.powi(-700);
    let curve = bezier(&[
        (point(Real::MAX, 0., 0.), small),
        (point(0., 0., 0.), 1.),
        (point(0., 0., 0.), 2_f64.powi(700)),
    ]);
    assert_eq!(
        curve.evaluate(2_f64.powi(-350)).unwrap().x(),
        Real::MAX * small
    );
}

#[test]
fn curve_range_loss_retains_finite_derivatives_after_coordinate_underflow() {
    let a = 1e-200;
    let curve = bezier(&[(point(0., 0., 0.), 1.), (point(a, 0., 0.), a)]);
    let jet = curve.evaluate_with_second_derivative(1.).unwrap();
    assert_eq!(jet.0, point(a, 0., 0.));
    assert_eq!(jet.1.to_array(), [1., 0., 0.]);
    assert_eq!(jet.2.to_array(), [2. / a, 0., 0.]);
    assert_eq!(curve.evaluate_with_derivative(1.).unwrap(), (jet.0, jet.1));
}

#[test]
fn curve_range_loss_retains_constant_jets_after_signed_weight_cancellation() {
    let p = point(1., 2., 3.);
    let curve = bezier(&[(p, Real::MAX), (p, Real::MIN_POSITIVE), (p, -Real::MAX)]);
    let jet = curve.evaluate_with_second_derivative(0.5).unwrap();
    assert_eq!(jet.0, p);
    assert_eq!(jet.1.to_array(), [0.; 3]);
    assert_eq!(jet.2.to_array(), [0.; 3]);
    assert!(matches!(
        curve.tangent_at_on_side(0.5, ParameterSide::Right),
        Err(GeometryError::Degenerate { .. })
    ));
}

#[test]
fn curve_range_loss_keeps_tangents_when_speed_underflows_or_the_point_is_stationary() {
    let a = 1e-200;
    let endpoint = point(a, 2. * a, -a);
    let expected = Vector3::try_new(1., 2., -1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    for controls in [
        vec![(point(0., 0., 0.), 1.), (endpoint, a)],
        vec![
            (point(0., 0., 0.), 1.),
            (point(0., 0., 0.), 1.),
            (endpoint, a),
        ],
    ] {
        let curve = bezier(&controls);
        let tangent = curve.tangent_at_on_side(0., ParameterSide::Right).unwrap();
        for (actual, expected) in tangent
            .as_vector()
            .to_array()
            .into_iter()
            .zip(expected.as_vector().to_array())
        {
            assert!((actual - expected).abs() < 2e-15);
        }
    }
}

#[test]
fn curve_range_loss_orients_all_orders_of_stationary_endpoint_tangents() {
    let a = 1e-200;
    let endpoint = point(a, 2. * a, -a);
    let expected = Vector3::try_new(1., 2., -1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    for degree in 1..=6 {
        for sign in [-1., 1.] {
            let mut controls = vec![(point(0., 0., 0.), sign); degree];
            controls.push((endpoint, sign * a));
            let curve = bezier(&controls);
            let reversed = curve.reversed().unwrap();
            for side in [ParameterSide::Left, ParameterSide::Right] {
                for (source, parameter, expected) in [
                    (&curve, *curve.domain().start(), expected),
                    (&reversed, *reversed.domain().end(), expected.opposite()),
                ] {
                    let actual = source
                        .tangent_at_on_side(parameter, side)
                        .unwrap()
                        .as_vector();
                    for (a, b) in actual
                        .to_array()
                        .into_iter()
                        .zip(expected.as_vector().to_array())
                    {
                        assert!(
                            (a - b).abs() < 2e-15,
                            "degree {degree}, sign {sign}, side {side:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn curve_range_loss_preserves_tangent_direction_when_requested_speed_overflows() {
    let curve = bezier(&[
        (point(0., 0., 0.), 1.),
        (point(1., 0., 0.), Real::from_bits(1)),
    ]);
    assert_eq!(curve.evaluate(1.).unwrap(), point(1., 0., 0.));
    assert!(matches!(
        curve.evaluate_with_derivative(1.),
        Err(GeometryError::NonFinite { .. })
    ));
    assert_eq!(
        curve
            .tangent_at_on_side(1., ParameterSide::Right)
            .unwrap()
            .as_vector()
            .to_array(),
        [1., 0., 0.]
    );
}

#[test]
fn curve_range_loss_keeps_independent_full_order_limits_and_validation() {
    let controls = [
        (point(0., 0., 0.), 1.),
        (point(1e-200, 0., 0.), 1e-200),
        (point(2., 3., 4.), Real::MAX),
        (point(2., 3., 4.), Real::MIN_POSITIVE),
    ];
    let curve = NurbsCurve::try_new_rational(
        1,
        controls
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .to_vec(),
        vec![0., 0., 1., 1., 2., 2.],
    )
    .unwrap();
    let original = curve.clone();
    let left = curve
        .evaluate_with_second_derivative_on_side(1., ParameterSide::Left)
        .unwrap();
    assert_eq!(left.0, point(1e-200, 0., 0.));
    assert_eq!(left.1.to_array(), [1., 0., 0.]);
    assert_eq!(left.2.to_array(), [2e200, 0., 0.]);
    let right = curve
        .evaluate_with_second_derivative_on_side(1., ParameterSide::Right)
        .unwrap();
    assert_eq!(right.0, point(2., 3., 4.));
    assert_eq!(right.1.to_array(), [0.; 3]);
    assert_eq!(right.2.to_array(), [0.; 3]);
    assert_eq!(curve.evaluate(1.).unwrap(), right.0);
    for t in [-1., 3., Real::INFINITY, Real::NEG_INFINITY, Real::NAN] {
        assert!(curve.evaluate(t).is_err());
        assert!(curve.evaluate_with_second_derivative(t).is_err());
        assert!(curve.tangent_at_on_side(t, ParameterSide::Right).is_err());
    }
    assert_eq!(curve, original);
}

#[test]
fn curve_range_loss_distinguishes_true_poles_from_constant_finite_jets() {
    let p = point(1., 2., 3.);
    let curve = bezier(&[
        (p, Real::MAX),
        (p, Real::MIN_POSITIVE),
        (p, -Real::MIN_POSITIVE),
        (p, -Real::MAX),
    ]);
    assert_eq!(
        curve.evaluate(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(
        curve.evaluate_with_second_derivative(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(
        curve.tangent_at_on_side(0.5, ParameterSide::Right),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    for t in [0., 0.25, 0.75, 1.] {
        let jet = curve.evaluate_with_second_derivative(t).unwrap();
        assert_eq!(jet.0, p);
        assert_eq!(jet.1.to_array(), [0.; 3]);
        assert_eq!(jet.2.to_array(), [0.; 3]);
    }
}

#[test]
fn mixed_weight_spans_preserve_constant_jets_before_denominator_cancellation() {
    let p = point(1., 2., 3.);
    let curve = bezier(&[(p, 1.), (p, 2_f64.powi(-100)), (p, -1.)]);
    let controls = curve.homogeneous_controls(2, true).unwrap();
    assert!(controls.homogeneous.iter().all(|h| h[3].is_normal()));
    assert!(controls.needs_exact);
    assert_eq!(curve.evaluate(0.5).unwrap(), p);
    let jet = curve.evaluate_with_second_derivative(0.5).unwrap();
    assert_eq!(jet.0, p);
    assert_eq!(jet.1.to_array(), [0.; 3]);
    assert_eq!(jet.2.to_array(), [0.; 3]);
}

#[test]
fn curve_exact_recovery_keeps_tangents_on_subnormal_parameter_domains() {
    for sign in [-1., 1.] {
        let curve = NurbsCurve::try_new_rational(
            1,
            [point(0., 0., 0.), point(1., 2., -1.)]
                .map(|p| WeightedPoint3::try_new(p, sign).unwrap())
                .to_vec(),
            vec![0., 0., Real::from_bits(1), Real::from_bits(1)],
        )
        .unwrap();
        let expected = Vector3::try_new(1., 2., -1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        for t in [0., Real::from_bits(1)] {
            assert!(curve.evaluate_with_derivative(t).is_err());
            assert_eq!(
                curve.tangent_at_on_side(t, ParameterSide::Right).unwrap(),
                expected
            );
        }
    }
}

#[test]
fn curve_exact_recovery_retains_constant_jets_with_overflowing_intermediate_factors() {
    let p = point(1e300, -1e300, 3.);
    let tiny = Real::from_bits(1);
    let curve = NurbsCurve::try_new_rational(
        1,
        [1., 2.]
            .map(|w| WeightedPoint3::try_new(p, w).unwrap())
            .to_vec(),
        vec![0., 0., tiny, tiny],
    )
    .unwrap();
    assert!(!curve.homogeneous_controls(1, true).unwrap().needs_exact);
    for t in [0., tiny] {
        let jet = curve.evaluate_with_second_derivative(t).unwrap();
        assert_eq!(jet.0, p);
        assert_eq!(jet.1.to_array(), [0.; 3]);
        assert_eq!(jet.2.to_array(), [0.; 3]);
        assert!(matches!(
            curve.tangent_at_on_side(t, ParameterSide::Right),
            Err(GeometryError::Degenerate { .. })
        ));
    }
}

#[test]
fn mixed_weight_native_jets_recognize_a_pole_hidden_by_rounded_blend_ratios() {
    for sign in [1., -1.] {
        let curve = NurbsCurve::try_new_rational(
            1,
            [(point(0., 0., 0.), sign), (point(1., 0., 0.), -2. * sign)]
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .to_vec(),
            vec![0., 0., 1.5, 1.5],
        )
        .unwrap();
        assert_eq!(
            curve.evaluate(0.5),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        assert_eq!(
            curve.evaluate_with_derivative(0.5),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        assert_eq!(
            curve.evaluate_with_second_derivative(0.5),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        assert_eq!(
            curve.tangent_at_on_side(0.5, ParameterSide::Right),
            Err(GeometryError::ZeroWeightAtParameter)
        );
        for (t, x, second) in [(0.25, -2. / 3., -128. / 3.), (0.75, 2., 128. / 3.)] {
            let (p, d, dd) = curve.evaluate_with_second_derivative(t).unwrap();
            assert_eq!(p, point(x, 0., 0.));
            assert_eq!(d.to_array(), [-16. / 3., 0., 0.]);
            assert_eq!(dd.to_array(), [second, 0., 0.]);
        }
    }
}

#[test]
fn common_sign_weight_spans_keep_the_fast_control_path() {
    for sign in [1., -1.] {
        let curve = bezier(&[
            (point(0., 0., 0.), sign),
            (point(1., 2., 0.), 2. * sign),
            (point(2., 0., 0.), 3. * sign),
        ]);
        assert!(!curve.homogeneous_controls(2, true).unwrap().needs_exact);
    }
}
