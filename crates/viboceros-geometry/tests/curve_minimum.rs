use viboceros_geometry::{
    CurveMinimumDistance, LineSegment, NurbsCurve, ParameterSide, Point3, Tolerance, WeightedPoint3,
};

fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn tolerance(e: f64) -> Tolerance {
    Tolerance::try_new(e, 1e-12, 1e-12).unwrap()
}
fn line(a: [f64; 3], b: [f64; 3]) -> NurbsCurve {
    LineSegment::try_new(point(a), point(b), Tolerance::NUMERICAL_VALIDATION)
        .unwrap()
        .to_nurbs()
        .unwrap()
}
fn check(a: &NurbsCurve, b: &NurbsCurve, expected: f64, epsilon: f64) -> CurveMinimumDistance {
    let before = [a.clone(), b.clone()];
    let result = a
        .minimum_distance_with_curve(b, tolerance(epsilon))
        .unwrap()
        .expect("complete-image minimum should certify");
    assert!(
        (result.upper_distance - expected).abs() <= 2. * epsilon,
        "{result:?}, expected {expected}"
    );
    assert_eq!(result.maximum_error, epsilon);
    for (i, curve) in [a, b].into_iter().enumerate() {
        assert!(curve.domain().contains(&result.parameters[i]));
        let p = curve
            .evaluate_on_side(result.parameters[i], result.sides[i])
            .unwrap();
        assert!(p.distance_to(result.points[i]).unwrap() <= epsilon.max(1e-14));
    }
    assert_eq!([a.clone(), b.clone()], before);
    result
}

#[test]
fn finite_segment_minima_include_intersections_skew_pairs_and_ends() {
    let a = line([-2., 0., 0.], [2., 0., 0.]);
    for (b, expected) in [
        (line([0., -2., 0.], [0., 2., 0.]), 0.),
        (line([0., -2., 3.], [0., 2., 3.]), 3.),
        (line([5., 2., 0.], [8., 2., 0.]), 13_f64.sqrt()),
    ] {
        for (a, b) in [(&a, &b), (&b, &a)] {
            check(a, b, expected, 1e-10);
        }
    }
}

#[test]
fn rotated_parallel_curves_do_not_require_uniform_pair_subdivision() {
    let a = line([0., 0., 0.], [3000., 4000., 0.]);
    let b = line([-4., 3., 0.], [2996., 4003., 0.]);
    check(&a, &b, 5., 1e-10);
}

#[test]
fn polynomial_and_rational_curved_pairs_find_interior_minima() {
    let a = NurbsCurve::try_new(
        2,
        vec![
            point([-1., 1., 0.]),
            point([0., -1., 0.]),
            point([1., 1., 0.]),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let b = line([-2., -2., 0.], [2., -2., 0.]);
    let r = check(&a, &b, 2., 1e-9);
    assert!((r.parameters[0] - 0.5).abs() < 1e-6);
    let quarter = NurbsCurve::try_new_rational(
        2,
        [[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
            .into_iter()
            .zip([1., 0.5_f64.sqrt(), 1.])
            .map(|(p, w)| WeightedPoint3::try_new(point(p), w).unwrap())
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    check(
        &quarter,
        &line([2., 2., -1.], [2., 2., 1.]),
        8_f64.sqrt() - 1.,
        1e-9,
    );
}

#[test]
fn complete_image_includes_short_spans_and_one_sided_jump_limits() {
    let points = (0..=1024)
        .map(|i| point([i as f64, if i == 723 { 0.125 } else { 2. }, 0.]))
        .collect();
    let mut knots = vec![0.];
    knots.extend((0..=1024).map(|i| i as f64));
    knots.push(1024.);
    let a = NurbsCurve::try_new(1, points, knots).unwrap();
    check(&a, &line([0., 0., 0.], [1024., 0., 0.]), 0.125, 1e-9);
    let jump = NurbsCurve::try_new(
        1,
        vec![
            point([-2., 3., 0.]),
            point([0., 0., 0.]),
            point([10., 10., 0.]),
            point([11., 11., 0.]),
        ],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let b = line([0., 0., -1.], [0., 0., 1.]);
    let r = check(&jump, &b, 0., 1e-100);
    assert_eq!(r.parameters[0], 0.5);
    assert_eq!(r.sides[0], ParameterSide::Left);
}

#[test]
fn an_inaccurate_external_seed_is_refined_and_never_accepted_as_global_evidence() {
    let a = line([-2., 0., 0.], [2., 0., 0.]);
    let b = line([0., -2., 3.], [0., 2., 3.]);
    let r = a
        .minimum_distance_with_curve_seeded(&b, tolerance(1e-10), [0., 0.])
        .unwrap()
        .unwrap();
    assert!(r.upper_distance - 3. <= 1e-10);
    assert!(
        a.minimum_distance_with_curve_seeded(&b, tolerance(1e-10), [f64::NAN, 0.])
            .is_err()
    );
}

#[test]
fn unclamped_active_domains_and_signed_rational_gauges_are_preserved() {
    let a = NurbsCurve::try_new(
        2,
        vec![
            point([-10., 2., 0.]),
            point([0., 2., 0.]),
            point([10., 2., 0.]),
        ],
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let b = line([20., 0., 0.], [21., 0., 0.]);
    check(&a, &b, 229_f64.sqrt(), 1e-10);
    for gauge in [-1e-200, 1e200] {
        let c = NurbsCurve::try_new_rational(
            a.degree(),
            a.control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), gauge).unwrap())
                .collect(),
            a.knots().to_vec(),
        )
        .unwrap();
        check(&c, &b, 229_f64.sqrt(), 1e-10);
    }
    let shifted = line([-2., 0., 0.], [2., 0., 0.]);
    let shifted = NurbsCurve::try_new(
        1,
        shifted.control_points().iter().map(|p| p.point()).collect(),
        vec![1e12, 1e12, 1e12 + 8., 1e12 + 8.],
    )
    .unwrap();
    let r = check(&shifted, &line([0., -2., 3.], [0., 2., 3.]), 3., 1e-10);
    assert_eq!(r.parameters[0], 1e12 + 4.);
}

#[test]
fn exact_bounds_avoid_overflow_underflow_and_large_origin_cancellation() {
    for scale in [2_f64.powi(-1000), 1., 2_f64.powi(500)] {
        let a = NurbsCurve::try_new(
            1,
            vec![point([-2. * scale, 0., 0.]), point([2. * scale, 0., 0.])],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let b = NurbsCurve::try_new(
            1,
            vec![
                point([0., -2. * scale, 3. * scale]),
                point([0., 2. * scale, 3. * scale]),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        check(&a, &b, 3. * scale, scale * 1e-10);
    }
    let base = 2_f64.powi(500);
    let step = 2_f64.powi(460);
    let a = line([base, base, base], [base + 4. * step, base, base]);
    let b = line(
        [base + step, base + 2. * step, base],
        [base + 3. * step, base + 2. * step, base],
    );
    check(&a, &b, 2. * step, step * 1e-9);
}

#[test]
fn unsupported_or_unrepresentable_witnesses_decline_without_editing_sources() {
    let a = NurbsCurve::try_new(
        1,
        vec![point([-1., 0., 0.]), point([3., 0., 0.])],
        vec![1e16, 1e16, 1e16_f64.next_up(), 1e16_f64.next_up()],
    )
    .unwrap();
    let b = line([0., 0., -1.], [0., 0., 1.]);
    let before = a.clone();
    assert!(
        a.minimum_distance_with_curve(&b, tolerance(1e-12))
            .unwrap()
            .is_none()
    );
    assert_eq!(a, before);
    let mixed = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(point([-1., 0., 0.]), 1.).unwrap(),
            WeightedPoint3::try_new(point([1., 0., 0.]), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        mixed
            .minimum_distance_with_curve(&b, tolerance(1e-9))
            .unwrap()
            .is_none()
    );
    let high =
        NurbsCurve::try_clamped_uniform(17, (0..18).map(|i| point([i as f64, 0., 0.])).collect())
            .unwrap();
    assert!(
        high.minimum_distance_with_curve(&b, tolerance(1e-9))
            .unwrap()
            .is_none()
    );
}
