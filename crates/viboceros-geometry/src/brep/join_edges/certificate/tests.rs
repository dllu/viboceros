use super::*;
use num_rational::BigRational as R;
use num_traits::Zero;

fn p(a: [Real; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn r(a: Real) -> R {
    R::from_float(a).unwrap()
}

#[test]
fn independent_pair_distance_order_matches_exact_rationals_at_all_float_scales() {
    let mut state = 348719_u64;
    for _ in 0..512 {
        let points: [Point3; 4] = std::array::from_fn(|_| {
            p(std::array::from_fn(|_| {
                loop {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let value = Real::from_bits(state);
                    if value.is_finite() {
                        break value;
                    }
                }
            }))
        });
        let squared = |a: Point3, b: Point3| -> R {
            a.to_array()
                .into_iter()
                .zip(b.to_array())
                .map(|(a, b)| {
                    let d = r(a) - r(b);
                    &d * &d
                })
                .sum()
        };
        assert_eq!(
            compare_pair_distances([points[0], points[1]], [points[2], points[3]]),
            squared(points[0], points[1]).cmp(&squared(points[2], points[3]))
        );
    }
    let tiny = Real::from_bits(1);
    assert_eq!(
        compare_pair_distances(
            [p([0.; 3]), p([1., tiny, 0.])],
            [p([100., 0., 0.]), p([101., 0., 0.])]
        ),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn distance_predicate_and_bounds_match_independent_exact_rationals() {
    let mut state = 157_u64;
    for i in 0..512 {
        let mut next = || loop {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = Real::from_bits(state);
            if x.is_finite() {
                break x;
            }
        };
        let a = p(std::array::from_fn(|_| next()));
        let b = p(std::array::from_fn(|_| next()));
        let limit = if i % 2 == 0 {
            a.distance_to(b).unwrap_or(Real::MAX)
        } else {
            next().abs()
        };
        let squared = a
            .to_array()
            .into_iter()
            .zip(b.to_array())
            .fold(R::zero(), |s, (a, b)| {
                let d = r(a) - r(b);
                s + &d * &d
            });
        let bound = point_bound(a, b, limit);
        assert_eq!(bound.is_some(), squared <= r(limit) * r(limit));
        if let Some(bound) = bound {
            assert!(bound <= limit);
            assert!(squared <= r(bound) * r(bound));
        }
    }
}

#[test]
fn distance_retains_subnormals_does_not_expand_with_translation_and_is_inclusive() {
    let tiny = Real::from_bits(1);
    for scale in [tiny, 1e-280, 1., 1e280, Real::MAX] {
        assert_eq!(
            point_bound(p([0.; 3]), p([scale, 0., 0.]), scale),
            Some(scale)
        );
        assert!(point_bound(p([0.; 3]), p([scale, tiny, 0.]), scale).is_none());
    }
    assert!(point_bound(p([1e15, 0., 0.]), p([1e15, 1e-3, 0.]), 1e-4).is_none());
    assert_eq!(point_bound(p([0.; 3]), p([0.; 3]), 0.), Some(0.));
    assert!(point_bound(p([Real::MAX, 0., 0.]), p([-Real::MAX, 0., 0.]), Real::MAX).is_none());
}

#[test]
fn affine_knot_and_weight_predicates_match_exact_rationals() {
    let mut state = 1789_u64;
    for _ in 0..256 {
        let values: [Real; 6] = std::array::from_fn(|_| {
            loop {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let x = Real::from_bits(state);
                if x.is_finite() {
                    break x;
                }
            }
        });
        let [a, a0, a1, b, b0, b1] = values;
        assert_eq!(
            same_fraction(a, a0, a1, b, b0, b1),
            (r(a) - r(a0)) * (r(b1) - r(b0)) == (r(b) - r(b0)) * (r(a1) - r(a0))
        );
        assert_eq!(proportional(a, b, a0, b0), r(a) * r(b0) == r(b) * r(a0));
        assert!(same_fraction(a, a0, a1, a, a0, a1));
        assert!(same_fraction(a, a0, a1, -a, -a0, -a1));
        assert!(proportional(a, a, a0, a0));
    }
    assert!(same_fraction(1e-280, 0., 2e-280, 1e280, 0., 2e280));
    assert!(!same_fraction(
        1e-280,
        0.,
        2e-280,
        (1e280_f64).next_up(),
        0.,
        2e280
    ));
    assert!(proportional(1e280, 1e280, 1e280, 1e280));
    assert!(!proportional(
        1e-280,
        1e-280,
        1e-280,
        (1e-280_f64).next_up()
    ));
}

fn curve(weights: &[Real], knots: &[Real], shift: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        weights
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                WeightedPoint3::try_new(p([i as Real, (i % 2) as Real, shift]), w).unwrap()
            })
            .collect(),
        knots.to_vec(),
    )
    .unwrap()
}

#[test]
fn rational_certificates_accept_affine_domains_and_common_weight_scales() {
    let a = curve(&[1., 0.5, 2., 1.], &[0., 0., 0., 0.25, 1., 1., 1.], 0.);
    for scale in [1e-280, 1., 1e280, -2.] {
        let b = curve(
            &[scale, scale * 0.5, scale * 2., scale],
            &[1e9, 1e9, 1e9, 1e9 + 2., 1e9 + 8., 1e9 + 8., 1e9 + 8.],
            0.,
        );
        assert_eq!(curve_bound(&a, &b, false, 0.), Some(0.));
        assert_eq!(curve_bound(&a, &b.reversed().unwrap(), true, 0.), Some(0.));
        assert!(curve_bound(&a, &b, true, 0.).is_none());
    }
    let shifted = curve(&[1., 0.5, 2., 1.], a.knots(), 0.001);
    assert_eq!(curve_bound(&a, &shifted, false, 0.001), Some(0.001));
    assert!(curve_bound(&a, &shifted, false, (0.001_f64).next_down()).is_none());
}

#[test]
fn fast_basis_certificate_requires_exact_compatibility_and_bounds_every_control() {
    let a = curve(&[1., 0.5, 2., 1.], &[0., 0., 0., 0.25, 1., 1., 1.], 0.);
    let mut controls = a.control_points().to_vec();
    controls[1] = WeightedPoint3::try_new(p([1., 2., 0.]), 0.5).unwrap();
    let bowed = NurbsCurve::try_new_rational(2, controls, a.knots().to_vec()).unwrap();
    assert!(curve_bound(&a, &bowed, false, 0.01).is_none());
    let b = curve(
        &[1., 0.5, 2., 1.],
        &[0., 0., 0., (0.25_f64).next_up(), 1., 1., 1.],
        0.,
    );
    assert!(curve_bound(&a, &b, false, 1.).is_none());
    let b = curve(&[1., (0.5_f64).next_up(), 2., 1.], a.knots(), 0.);
    assert!(curve_bound(&a, &b, false, 1.).is_none());
}

#[test]
fn tolerance_addition_rounds_outward_and_rejects_overflow() {
    for (a, b) in [
        (0., 1.),
        (1., 0.),
        (1., 1e-300),
        (1e-300, 1e-300),
        (1e300, 1.),
    ] {
        let bound = add_bound(a, b).unwrap();
        assert!(r(bound) >= r(a) + r(b));
    }
    assert!(add_bound(Real::MAX, Real::MAX).is_err());
}

#[test]
fn straight_locus_certificate_rejects_breaks_bows_backtracking_and_mixed_weights() {
    let line = NurbsCurve::try_new(
        1,
        vec![p([0., 0., 0.]), p([3., 0., 0.])],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let make = |x: [Real; 4], y: Real, weights: [Real; 4]| {
        NurbsCurve::try_new_rational(
            3,
            x.into_iter()
                .enumerate()
                .map(|(i, x)| {
                    WeightedPoint3::try_new(p([x, if i == 1 { y } else { 0. }, 0.]), weights[i])
                        .unwrap()
                })
                .collect(),
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap()
    };
    for weights in [[1., 0.25, 2., 1.], [-1., -0.25, -2., -1.]] {
        let cubic = make([0., 1., 2., 3.], 0., weights);
        assert_eq!(curve_bound(&line, &cubic, false, 0.), Some(0.));
        assert_eq!(
            curve_bound(&line, &cubic.reversed().unwrap(), true, 0.),
            Some(0.)
        );
    }
    for bad in [
        make([0., 1., 2., 3.], Real::from_bits(1), [1.; 4]),
        make([0., 2., 1., 3.], 0., [1.; 4]),
        make([0., 1., 2., 3.], 0., [1., -0.25, 2., 1.]),
        NurbsCurve::try_new(
            1,
            [0., 1., 2., 3.]
                .into_iter()
                .map(|x| p([x, 0., 0.]))
                .collect(),
            vec![0., 0., 0.5, 0.5, 1., 1.],
        )
        .unwrap(),
        NurbsCurve::try_new(
            1,
            vec![p([0., 0., 0.]), p([3., 0., 0.])],
            vec![-1., 0., 1., 2.],
        )
        .unwrap(),
    ] {
        assert!(linear_endpoints(&bad).is_none());
        assert!(curve_bound(&line, &bad, false, 0.).is_none());
    }
}
