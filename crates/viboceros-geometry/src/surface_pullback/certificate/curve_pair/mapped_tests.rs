use super::*;
use crate::WeightedPoint3;

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn polygon(knots: Vec<Real>) -> NurbsCurve {
    NurbsCurve::try_new(1, vec![p(0., 0.), p(1., 2.), p(4., 0.)], knots).unwrap()
}

#[test]
fn different_piecewise_clocks_have_an_exact_complete_image_certificate() {
    let a = polygon(vec![2., 2., 4., 10., 10.]);
    let b = polygon(vec![-6., -6., 0., 2., 2.]);
    assert_eq!(a.parameter_curve_deviation_bound(&b, 0.1).unwrap(), None);
    let pairs = [[0., 0.], [0.25, 0.75], [1., 1.]];
    assert_eq!(
        a.mapped_curve_deviation_bound(&b, &pairs, 0.).unwrap(),
        Some(0.)
    );
    let inverse = pairs.map(|p| [p[1], p[0]]);
    assert_eq!(
        b.mapped_curve_deviation_bound(&a, &inverse, 0.).unwrap(),
        Some(0.)
    );
}

#[test]
fn every_knot_crossing_is_checked_in_both_curves() {
    // The supplied correspondence only matches endpoints. All interior source
    // knots, including the hidden bump's vertex, must still split the proof.
    let bump = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.), p(2., 2.), p(3., 0.), p(4., 0.)],
        vec![0., 0., 0.25, 0.5, 0.75, 1., 1.],
    )
    .unwrap();
    let line = NurbsCurve::try_new(1, vec![p(0., 0.), p(4., 0.)], vec![0., 0., 1., 1.]).unwrap();
    for (a, b) in [(&bump, &line), (&line, &bump)] {
        assert_eq!(
            a.mapped_curve_deviation_bound(b, &[[0., 0.], [1., 1.]], 1.)
                .unwrap(),
            None
        );
        assert_eq!(
            a.mapped_curve_deviation_bound(b, &[[0., 0.], [1., 1.]], 2.)
                .unwrap(),
            Some(2.)
        );
    }
}

#[test]
fn a_jump_hidden_on_the_left_of_a_mapped_knot_is_not_skipped() {
    let a = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.), p(2., 0.), p(3., 0.)],
        vec![0., 0., 0.25, 0.25, 1., 1.],
    )
    .unwrap();
    let b = NurbsCurve::try_new(
        1,
        vec![p(0., 0.), p(1., 0.25), p(2., 0.), p(3., 0.)],
        vec![0., 0., 0.75, 0.75, 1., 1.],
    )
    .unwrap();
    let pairs = [[0., 0.], [0.25, 0.75], [1., 1.]];
    assert_eq!(
        a.mapped_curve_deviation_bound(&b, &pairs, 0.1).unwrap(),
        None
    );
    assert_eq!(
        a.mapped_curve_deviation_bound(&b, &pairs, 0.25).unwrap(),
        Some(0.25)
    );
}

#[test]
fn subnormal_error_and_independent_negative_weight_gauges_are_retained() {
    let tiny = Real::from_bits(1);
    let a = polygon(vec![0., 0., 0.25, 1., 1.]);
    let b = NurbsCurve::try_new_rational(
        1,
        [p(0., tiny), p(1., 2.), p(4., tiny)]
            .map(|p| WeightedPoint3::try_new(p, -7.).unwrap())
            .to_vec(),
        vec![0., 0., 0.75, 1., 1.],
    )
    .unwrap();
    let pairs = [[0., 0.], [0.25, 0.75], [1., 1.]];
    assert_eq!(
        a.mapped_curve_deviation_bound(&b, &pairs, 0.).unwrap(),
        None
    );
    assert_eq!(
        a.mapped_curve_deviation_bound(&b, &pairs, tiny).unwrap(),
        Some(tiny)
    );
}

#[test]
fn matching_five_samples_cannot_certify_an_intervening_polynomial_bump() {
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
    let line = NurbsCurve::try_new(1, vec![p(0., 0.), p(5., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let pairs = [0., 0.25, 0.5, 0.75, 1.].map(|f| [f, f]);
    for pair in pairs {
        assert_eq!(
            bump.evaluate(pair[0]).unwrap(),
            line.evaluate(pair[1]).unwrap()
        );
    }
    assert_eq!(
        bump.mapped_curve_deviation_bound(&line, &pairs, 0.1)
            .unwrap(),
        None
    );
}

#[test]
fn invalid_or_partial_maps_are_rejected_before_certification() {
    let a = polygon(vec![0., 0., 0.25, 1., 1.]);
    for pairs in [
        vec![],
        vec![[0., 0.]],
        vec![[0., 0.], [0.5, 1.]],
        vec![[0., 0.], [1., 0.]],
        vec![[0., 0.], [0.5, 0.5], [0.5, 0.75], [1., 1.]],
        vec![[0., 0.], [0.5, 0.75], [0.75, 0.5], [1., 1.]],
        vec![[0., 0.], [Real::NAN, 0.5], [1., 1.]],
        vec![[0., 0.], [Real::INFINITY, 0.5], [1., 1.]],
        (0..4098).map(|i| [i as Real / 4097.; 2]).collect(),
    ] {
        assert_eq!(
            a.mapped_curve_deviation_bound(&a, &pairs, 0.),
            Err(GeometryError::InvalidCurveCorrespondence)
        );
    }
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert_eq!(
            a.mapped_curve_deviation_bound(&a, &[[0., 0.], [1., 1.]], limit),
            Err(GeometryError::InvalidTolerance)
        );
    }
}
