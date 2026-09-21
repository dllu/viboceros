use super::*;

fn retime_bezier(c: &NurbsCurve, factor: Real, gauge: Real) -> NurbsCurve {
    assert_eq!(c.control_points().len(), c.degree() + 1);
    let weights = c
        .control_points()
        .iter()
        .enumerate()
        .map(|(i, c)| c.weight() * factor.powi(i as i32) * gauge)
        .collect::<Vec<_>>();
    curve(
        &c.control_points()
            .iter()
            .map(|c| c.point().to_array())
            .collect::<Vec<_>>(),
        &weights,
        c.degree(),
        c.knots(),
    )
}

#[test]
fn projective_speeds_certify_the_same_entire_curve_without_changing_controls() {
    let original = quadratic();
    for factor in [0.125, 0.5, 2., 8., 1e-100, 1e100] {
        for gauge in [1., -2.] {
            let b = retime_bezier(&original, factor, gauge);
            for reversed in [false, true] {
                let b = if reversed {
                    b.reversed().unwrap()
                } else {
                    b.clone()
                };
                // Non-power-of-two factors independently round their powers.
                let limit = if factor == 1e-100 || factor == 1e100 {
                    1e-12
                } else {
                    0.
                };
                let result = certify(&original, &b, reversed, limit).unwrap();
                assert!(result <= limit);
                assert_eq!(certify(&original, &b, !reversed, limit), None);
            }
        }
    }
}

#[test]
fn degree_elevation_and_independent_knot_refinement_survive_projective_alignment() {
    let a = quadratic();
    let cubic = curve(
        &[[0., 0., 0.], [10., 10., 0.], [20., 10., 0.], [30., 0., 0.]],
        &[1.; 4],
        3,
        &[0., 0., 0., 0., 1., 1., 1., 1.],
    );
    for b in [
        retime_bezier(&cubic, 2., 1.),
        retime_bezier(&cubic, 0.25, -4.),
    ] {
        assert_eq!(certify(&a, &b, false, 0.), Some(0.));
        // Knot insertion rounds projected controls. Those actual binary64
        // curves need a nonzero bound, not an assumed exact-edit certificate.
        let a = a
            .try_insert_knot(0.25, 1)
            .unwrap()
            .try_insert_knot(0.75, 1)
            .unwrap();
        let b = b
            .try_insert_knot(0.5, 2)
            .unwrap()
            .try_insert_knot(0.125, 1)
            .unwrap();
        assert!(certify(&a, &b, false, 1e-12).unwrap() <= 1e-12);
        assert!(certify(&b, &a, false, 1e-12).unwrap() <= 1e-12);
    }
}

#[test]
fn stationary_endpoints_can_use_weight_proposals_but_not_as_acceptance() {
    let a = curve(
        &[
            [0., 0., 0.],
            [0., 0., 0.],
            [2., 3., 0.],
            [4., 0., 0.],
            [4., 0., 0.],
        ],
        &[1.; 5],
        4,
        &[0., 0., 0., 0., 0., 1., 1., 1., 1., 1.],
    );
    let b = retime_bezier(&a, 2., 1.);
    assert_eq!(certify(&a, &b, false, 0.), Some(0.));
    let mut controls = b.control_points().to_vec();
    controls[2] =
        WeightedPoint3::try_new(Point3::try_new(2., 3., 1.).unwrap(), controls[2].weight())
            .unwrap();
    let wrong = NurbsCurve::try_new_rational(4, controls, b.knots().to_vec()).unwrap();
    assert_eq!(certify(&a, &wrong, false, 0.01), None);
}

#[test]
fn projective_span_composition_matches_independent_exact_global_basis() {
    let a = quadratic().try_insert_knot(0.25, 1).unwrap();
    let b = retime_bezier(&quadratic(), 4., 1.)
        .try_insert_knot(0.5, 1)
        .unwrap();
    let aa = extract::Spline::new(&a, false, &mut |_| Ok(()))
        .unwrap()
        .unwrap();
    let bb = extract::Spline::new(&b, false, &mut |_| Ok(()))
        .unwrap()
        .unwrap();
    let maps = parameter_map::candidates(&aa, &bb, &mut |_| Ok(())).unwrap();
    assert!(!maps.is_empty());
    let map = &maps[0];
    for i in 0..=256 {
        let t = rational(i as Real / 256.);
        assert_eq!(map.inverse(&map.apply(&t)), t);
        let av = exact_value(&a, &t);
        let bv = exact_value(&b, &map.apply(&t));
        let square: Rational = av
            .iter()
            .zip(bv)
            .map(|(a, b)| {
                let d = a - b;
                &d * &d
            })
            .sum();
        assert!(square <= rational(1e-12) * rational(1e-12));
    }
    let mut budget = Budget(MAX_WORK);
    let upper = mapped_bound(&aa, &bb, map, 1e-12, true, &mut |n| budget.charge(n))
        .unwrap()
        .unwrap();
    assert!(upper <= 1e-12);
}

#[test]
fn offset_projective_curves_keep_a_proven_nonzero_bound_and_charge_all_attempts() {
    let a = quadratic();
    let source = retime_bezier(&a, 4., 1.);
    let b = curve(
        &source
            .control_points()
            .iter()
            .map(|c| {
                let mut p = c.point().to_array();
                p[2] = 0.0005;
                p
            })
            .collect::<Vec<_>>(),
        &source
            .control_points()
            .iter()
            .map(|c| c.weight())
            .collect::<Vec<_>>(),
        2,
        source.knots(),
    );
    assert_eq!(certify(&a, &b, false, 0.0005), Some(0.0005));
    assert_eq!(certify(&a, &b, false, 0.0005_f64.next_down()), None);
    let mut budget = Budget(MAX_WORK);
    let bound = refined_curve_bound(&a, &b, false, Real::MAX, |n| budget.charge(n))
        .unwrap()
        .unwrap();
    assert_eq!(bound, 0.0005);
    let before = (a.clone(), b.clone());
    let mut budget = Budget(100);
    assert!(whole_curve_bound(&a, &b, false, 0.0005, |n| budget.charge(n)).is_err());
    assert_eq!((a, b), before);
}

#[test]
fn identical_nonprojective_loci_remain_an_explicit_limit_not_a_sampled_match() {
    let a = quadratic();
    // A(t)=(30t,30t(1-t),0), B(s)=A(s^2). Squaring is not a positive
    // endpoint-preserving projective bijection with nonzero endpoint speed.
    let b = curve(
        &[
            [0., 0., 0.],
            [0., 0., 0.],
            [5., 5., 0.],
            [15., 15., 0.],
            [30., 0., 0.],
        ],
        &[1.; 5],
        4,
        &[0., 0., 0., 0., 0., 1., 1., 1., 1., 1.],
    );
    for i in 0..=64 {
        let t = rational(i as Real / 64.);
        assert_eq!(exact_value(&a, &(&t * &t)), exact_value(&b, &t));
    }
    assert_eq!(certify(&a, &b, false, 1e-9), None);
}

#[test]
fn near_projective_profile_requires_at_least_its_true_normal_gap() {
    let a = quadratic();
    let mut b = retime_bezier(&a, 2., 1.);
    let mut controls = b.control_points().to_vec();
    controls[1] = WeightedPoint3::try_new(Point3::try_new(15., 15., 0.0005).unwrap(), 2.).unwrap();
    b = NurbsCurve::try_new_rational(2, controls, b.knots().to_vec()).unwrap();
    let peak = exact_value(&b, &Rational::new(1.into(), 3.into()));
    assert_eq!(peak[2], rational(0.0005) / rational(2.));
    // Every point of A lies in z=0. No correspondence can shrink this normal
    // separation. Rhino's archived component tolerance is smaller than it.
    assert!(peak[2] > rational(0.00024793388429752067));
    let mut budget = Budget(MAX_WORK);
    let upper = refined_curve_bound(&a, &b, false, Real::MAX, |n| budget.charge(n))
        .unwrap()
        .unwrap();
    assert_eq!(upper, 0.00025);
}
