use super::*;
use crate::{ParameterSide, WeightedPoint3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn line() -> NurbsCurve {
    NurbsCurve::try_new(1, vec![p(0., 0.), p(3., 6.)], vec![2., 2., 5., 5.]).unwrap()
}

#[test]
fn equivalent_bases_domains_and_weight_gauges_have_exact_zero_bound() {
    let original = line();
    for gauge in [1., -7., 1e-250, -1e250] {
        let other = NurbsCurve::try_new_rational(
            3,
            [p(0., 0.), p(1., 2.), p(2., 4.), p(3., 6.)]
                .map(|p| WeightedPoint3::try_new(p, gauge).unwrap())
                .to_vec(),
            vec![-3., -3., -3., -3., 13., 13., 13., 13.],
        )
        .unwrap();
        assert_eq!(
            original
                .parameter_curve_deviation_bound(&other, 0.)
                .unwrap(),
            Some(0.)
        );
        assert_eq!(
            other
                .parameter_curve_deviation_bound(&original, 0.)
                .unwrap(),
            Some(0.)
        );
    }
}

#[test]
fn the_same_line_locus_with_a_different_rational_parameter_is_not_equal() {
    let original =
        NurbsCurve::try_new(1, vec![p(0., 0.), p(3., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let other = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(3., 0.), 2.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        original
            .parameter_curve_deviation_bound(&other, 0.49)
            .unwrap(),
        None
    );
    let bound = original
        .parameter_curve_deviation_bound(&other, 0.55)
        .unwrap()
        .unwrap();
    assert!(bound >= 3. * (3. - 2. * 2_f64.sqrt()));
    assert!(bound <= 0.55);
}

#[test]
fn both_limits_of_full_order_knots_are_checked() {
    let a = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.), p(2., 0.), p(3., 0.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let b = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.25), p(2., 0.), p(3., 0.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    assert_eq!(
        a.evaluate_on_side(0.5, ParameterSide::Right).unwrap(),
        b.evaluate_on_side(0.5, ParameterSide::Right).unwrap()
    );
    assert_eq!(a.parameter_curve_deviation_bound(&b, 0.1).unwrap(), None);
    assert_eq!(
        a.parameter_curve_deviation_bound(&b, 0.25).unwrap(),
        Some(0.25)
    );
}

#[test]
fn invalid_limits_and_unsupported_weight_signs_are_explicit() {
    let original = line();
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            original.parameter_curve_deviation_bound(&original, limit),
            Err(GeometryError::InvalidTolerance)
        );
    }
    let mixed = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
            WeightedPoint3::try_new(p(1., 1.), -0.1).unwrap(),
            WeightedPoint3::try_new(p(2., 0.), 1.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        mixed.parameter_curve_deviation_bound(&mixed, 0.).unwrap(),
        None
    );
}

#[test]
fn exact_subnormal_distance_is_not_rounded_down_to_zero() {
    let tiny = Real::from_bits(1);
    let a = NurbsCurve::try_new(1, vec![p(0., 0.), p(1., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let b = NurbsCurve::try_new(1, vec![p(0., tiny), p(1., tiny)], vec![0., 0., 1., 1.]).unwrap();
    assert_eq!(a.parameter_curve_deviation_bound(&b, 0.).unwrap(), None);
    assert_eq!(
        a.parameter_curve_deviation_bound(&b, tiny).unwrap(),
        Some(tiny)
    );
}

#[test]
fn a_polynomial_bump_between_five_coincident_samples_is_detected() {
    // y(t) = 320 t(t-1/4)(t-1/2)(t-3/4)(t-1), represented
    // independently by exact integer Bernstein controls.
    let bump = NurbsCurve::try_new(
        5,
        [0., 6., -13., 13., -6., 0.]
            .into_iter()
            .enumerate()
            .map(|(i, y)| p(i as Real, y))
            .collect(),
        [vec![0.; 6], vec![1.; 6]].concat(),
    )
    .unwrap();
    let straight =
        NurbsCurve::try_new(1, vec![p(0., 0.), p(5., 0.)], vec![0., 0., 1., 1.]).unwrap();
    for t in [0., 0.25, 0.5, 0.75, 1.] {
        assert_eq!(bump.evaluate(t).unwrap(), straight.evaluate(t).unwrap());
    }
    assert!(bump.evaluate(0.125).unwrap().y() > 1.);
    assert_eq!(
        bump.parameter_curve_deviation_bound(&straight, 0.1)
            .unwrap(),
        None
    );
}
