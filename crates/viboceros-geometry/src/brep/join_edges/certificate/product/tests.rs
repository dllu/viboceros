use super::*;
mod projective;

fn curve(points: &[[Real; 3]], weights: &[Real], degree: usize, knots: &[Real]) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        degree,
        points
            .iter()
            .zip(weights)
            .map(|(&p, &w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
            .collect(),
        knots.to_vec(),
    )
    .unwrap()
}

fn quadratic() -> NurbsCurve {
    curve(
        &[[0., 0., 0.], [15., 15., 0.], [30., 0., 0.]],
        &[1.; 3],
        2,
        &[0., 0., 0., 1., 1., 1.],
    )
}

fn certify(a: &NurbsCurve, b: &NurbsCurve, reversed: bool, limit: Real) -> Option<Real> {
    let mut budget = Budget(MAX_WORK);
    whole_curve_bound(a, b, reversed, limit, |n| budget.charge(n)).unwrap()
}

#[test]
fn different_degrees_knots_and_nonproportional_weights_can_be_exactly_equal() {
    let a = quadratic();
    let elevated = curve(
        &[[0., 0., 0.], [10., 10., 0.], [20., 10., 0.], [30., 0., 0.]],
        &[1.; 4],
        3,
        &[0., 0., 0., 0., 1., 1., 1., 1.],
    );
    // Multiply the quadratic's homogeneous polynomials by 3(1+t).
    let rational_cubic = curve(
        &[[0., 0., 0.], [7.5, 7.5, 0.], [18., 12., 0.], [30., 0., 0.]],
        &[3., 4., 5., 6.],
        3,
        elevated.knots(),
    );
    let inserted = a.try_insert_knot(0.5, 1).unwrap();
    for b in [&elevated, &rational_cubic, &inserted] {
        assert!(curve_bound(&a, b, false, 0.).is_none());
        assert_eq!(certify(&a, b, false, 0.), Some(0.));
        let reversed = b.reversed().unwrap();
        assert_eq!(certify(&a, &reversed, true, 0.), Some(0.));
        assert_eq!(certify(&a, &reversed, false, 0.), None);
        for gauge in [Real::from_bits(1), -2., 1e-280, 1e280] {
            let b = curve(
                &b.control_points()
                    .iter()
                    .map(|c| c.point().to_array())
                    .collect::<Vec<_>>(),
                &b.control_points()
                    .iter()
                    .map(|c| c.weight() * gauge)
                    .collect::<Vec<_>>(),
                b.degree(),
                b.knots(),
            );
            // Decimal gauges may independently round individual weights;
            // that changes the represented rational curve by a tiny amount.
            assert!(certify(&a, &b, false, 1e-12).unwrap() <= 1e-12);
        }
    }
}

#[test]
fn normalized_knots_do_not_lose_tiny_spans_or_extreme_parameter_origins() {
    let a = quadratic();
    let refined = a
        .try_insert_knot(0.25, 1)
        .unwrap()
        .try_insert_knot(0.5, 1)
        .unwrap();
    for (start, scale) in [(1e15, 8.), (0., 1e-280), (-1e280, 2e280)] {
        let knots = refined
            .knots()
            .iter()
            .map(|k| start + k * scale)
            .collect::<Vec<_>>();
        let b = NurbsCurve::try_new_rational(2, refined.control_points().to_vec(), knots).unwrap();
        // The mathematical affine image uses the actual binary64 knots.
        let bound = certify(&a, &b, false, 1e-12).unwrap();
        assert!(bound <= 1e-12);
    }
}

#[test]
fn subdivision_can_certify_a_curve_even_when_its_control_hull_exceeds_limit() {
    let a = quadratic();
    let mut points = a
        .control_points()
        .iter()
        .map(|c| c.point().to_array())
        .collect::<Vec<_>>();
    points[1][2] = 0.004;
    let b = curve(&points, &[1.; 3], 2, a.knots());
    assert_eq!(curve_bound(&a, &b, false, 0.002), None);
    assert_eq!(certify(&a, &b, false, 0.002), Some(0.002));
    assert_eq!(certify(&a, &b, false, 0.002_f64.next_down()), None);
    assert_eq!(certify(&a, &b, false, 0.), None);
}

// Independent global Cox-de Boor basis evaluation, not the local polar-form
// extraction or Bernstein-product implementation under test.
fn exact_value(curve: &NurbsCurve, t: &Rational) -> [Rational; 3] {
    let knots = curve
        .knots()
        .iter()
        .map(|&k| rational(k))
        .collect::<Vec<_>>();
    let mut basis = (0..knots.len() - 1)
        .map(|i| {
            rational(
                if (&knots[i] <= t && t < &knots[i + 1])
                    || (*t == rational(*curve.domain().end())
                        && i + 1 == curve.control_points().len())
                {
                    1.
                } else {
                    0.
                },
            )
        })
        .collect::<Vec<_>>();
    for p in 1..=curve.degree() {
        basis = (0..basis.len() - 1)
            .map(|i| {
                let mut value = rational(0.);
                if knots[i + p] != knots[i] {
                    value += (t - &knots[i]) / (&knots[i + p] - &knots[i]) * &basis[i];
                }
                if knots[i + p + 1] != knots[i + 1] {
                    value += (&knots[i + p + 1] - t) / (&knots[i + p + 1] - &knots[i + 1])
                        * &basis[i + 1];
                }
                value
            })
            .collect();
    }
    let w: Rational = basis
        .iter()
        .zip(curve.control_points())
        .map(|(b, c)| b * rational(c.weight()))
        .sum();
    std::array::from_fn(|axis| {
        basis
            .iter()
            .zip(curve.control_points())
            .map(|(b, c)| b * rational(c.weight()) * rational(c.point().to_array()[axis]))
            .sum::<Rational>()
            / &w
    })
}

#[test]
fn extracted_spans_match_independent_exact_basis_at_knots_and_interior_points() {
    for (degree, knots) in [
        (2, vec![-2., -1., 0., 0.25, 0.5, 1., 2., 3.]),
        (2, vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.]),
        (3, vec![0., 0., 0., 0., 0.125, 0.75, 1., 1., 1., 1.]),
    ] {
        let count = knots.len() - degree - 1;
        let points = (0..count)
            .map(|i| [i as Real, (i * i % 7) as Real, (i % 3) as Real])
            .collect::<Vec<_>>();
        let weights = (0..count).map(|i| (i + 1) as Real / 4.).collect::<Vec<_>>();
        let c = curve(&points, &weights, degree, &knots);
        let spline = extract::Spline::new(&c, false, &mut |_| Ok(()))
            .unwrap()
            .unwrap();
        for span in degree..count {
            if knots[span] == knots[span + 1] {
                continue;
            }
            // Also test a proper subinterval, as needed for unequal knot partitions.
            let left = rational(knots[span]);
            let right = (&left + rational(knots[span + 1])) / rational(2.);
            let net = spline
                .extract(span, &left, &right, &mut |_| Ok(()))
                .unwrap();
            for j in 0..=8 {
                let t = rational(j as Real / 8.);
                let u = rational(1.) - &t;
                let mut h: H = std::array::from_fn(|_| rational(0.));
                for (i, p) in net.iter().enumerate() {
                    let basis = Rational::from_integer(binomial(degree, i).into())
                        * u.pow((degree - i) as i32)
                        * t.pow(i as i32);
                    for axis in 0..4 {
                        h[axis] += &basis * &p[axis];
                    }
                }
                let expected = exact_value(&c, &(&left + &t * (&right - &left)));
                for axis in 0..3 {
                    assert_eq!(&h[axis] / &h[3], expected[axis]);
                }
            }
        }
    }
}

#[test]
fn differing_rational_weights_have_certified_bounds_covering_exact_samples() {
    let a = quadratic();
    let b = curve(
        &[[0., 0., 0.], [15., 15., 0.], [30., 0., 0.]],
        &[1., 1.0001, 1.],
        2,
        a.knots(),
    );
    let mut budget = Budget(MAX_WORK);
    let aa = extract::Spline::new(&a, false, &mut |n| budget.charge(n))
        .unwrap()
        .unwrap();
    let bb = extract::Spline::new(&b, false, &mut |n| budget.charge(n))
        .unwrap()
        .unwrap();
    // This independent evaluator compares equal parameters. Request that
    // specific map, not the public locus certificate's best correspondence.
    let upper = mapped_bound(
        &aa,
        &bb,
        &parameter_map::Map::identity(),
        1.,
        true,
        &mut |n| budget.charge(n),
    )
    .unwrap()
    .unwrap();
    assert!(upper < 0.001);
    for i in 0..=256 {
        let t = rational(i as Real / 256.);
        let av = exact_value(&a, &t);
        let bv = exact_value(&b, &t);
        let squared: Rational = av
            .iter()
            .zip(bv)
            .map(|(a, b)| {
                let d = a - b;
                &d * &d
            })
            .sum();
        assert!(squared <= rational(upper) * rational(upper));
    }
    assert!(whole_curve_bound(&a, &b, false, 1., |_| Err(invalid("test budget"))).is_err());
    let before = (a.clone(), b.clone());
    let mut exhausted = Budget(50);
    assert!(whole_curve_bound(&a, &b, false, 1., |n| exhausted.charge(n)).is_err());
    assert_eq!((a, b), before);
}

#[test]
fn rational_multispan_difference_covers_independently_evaluated_curves() {
    let original = quadratic();
    let a = original.try_insert_knot(0.5, 1).unwrap();
    let b = curve(
        &[[0., 0., 0.], [15., 15., 0.0003], [30., 0., 0.]],
        &[1., 1.0001, 1.],
        2,
        original.knots(),
    )
    .try_insert_knot(0.25, 1)
    .unwrap()
    .try_insert_knot(0.75, 2)
    .unwrap();
    for reversed in [false, true] {
        let candidate = if reversed {
            b.reversed().unwrap()
        } else {
            b.clone()
        };
        assert!(certify(&a, &candidate, reversed, 0.002).is_some());
        let mut budget = Budget(MAX_WORK);
        let aa = extract::Spline::new(&a, false, &mut |n| budget.charge(n))
            .unwrap()
            .unwrap();
        let bb = extract::Spline::new(&candidate, reversed, &mut |n| budget.charge(n))
            .unwrap()
            .unwrap();
        let upper = mapped_bound(
            &aa,
            &bb,
            &parameter_map::Map::identity(),
            0.002,
            false,
            &mut |n| budget.charge(n),
        )
        .unwrap()
        .unwrap();
        for i in 0..=256 {
            let t = rational(i as Real / 256.);
            let av = exact_value(&a, &t);
            let bv = exact_value(&b, &t);
            let squared: Rational = av
                .iter()
                .zip(bv)
                .map(|(a, b)| {
                    let d = a - b;
                    &d * &d
                })
                .sum();
            assert!(squared <= rational(upper) * rational(upper));
        }
        assert_eq!(certify(&a, &candidate, reversed, 0.00001), None);
    }
}

#[test]
fn unresolved_hulls_at_the_depth_limit_do_not_become_matches() {
    let knots = [0., 0., 0., 0., 1., 1., 1., 1.];
    let a = curve(
        &[[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [3., 0., 0.]],
        &[1.; 4],
        3,
        &knots,
    );
    let b = curve(
        &[[0., 0., 0.], [1., 1., 0.], [2., 0., 0.], [3., 0., 0.]],
        &[1.; 4],
        3,
        &knots,
    );
    // max 3t(1-t)^2 = 4/9 at non-dyadic t=1/3. The tiny slack is too
    // small for sixteen subdivisions, despite being mathematically positive.
    let limit = (4_f64 / 9.).next_up();
    assert!(rational(limit) > Rational::new(4.into(), 9.into()));
    assert_eq!(certify(&a, &b, false, limit), None);
    assert!(certify(&a, &b, false, 0.445).is_some());
}

#[test]
fn exact_norm_bounds_do_not_overflow_or_erase_subnormal_residuals() {
    let tiny = Real::from_bits(1);
    let h = [
        rational(Real::MAX),
        rational(tiny),
        rational(0.),
        rational(1.),
    ];
    assert_eq!(refine::norm_bound(&h, Real::MAX), None);
    let h = [rational(tiny), rational(0.), rational(0.), rational(1.)];
    assert_eq!(refine::norm_bound(&h, 0.), None);
    assert_eq!(refine::norm_bound(&h, tiny), Some(tiny));
}
