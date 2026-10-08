use super::*;

fn encloses(i: I, r: &Rational) {
    assert!(
        rational(i.lo) <= *r && rational(i.hi) >= *r,
        "{i:?} misses {r}"
    );
}

#[test]
fn rounded_arithmetic_encloses_independent_exact_binary64_results() {
    let mut state = 0x7234ab951183u64;
    let mut tested = [0; 3];
    for _ in 0..3000 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = Real::from_bits(state);
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b = Real::from_bits(state);
        let (Some(x), Some(y)) = (I::point(a), I::point(b)) else {
            continue;
        };
        let (a, b) = (rational(a), rational(b));
        if let Some(result) = x.add(y) {
            encloses(result, &(&a + &b));
            tested[0] += 1;
        }
        if let Some(result) = x.mul(y) {
            encloses(result, &(&a * &b));
            tested[1] += 1;
        }
        if let Some(result) = x.div(y) {
            encloses(result, &(&a / &b));
            tested[2] += 1;
        }
    }
    assert!(tested.into_iter().all(|n| n > 1000));
}

#[test]
fn zero_identities_overflow_and_subnormal_boundaries_are_conservative() {
    assert!(I::point(Real::from_bits(1)).is_none());
    let a = I::point(Real::MIN_POSITIVE).unwrap();
    assert!(a.mul(I::point(0.5).unwrap()).is_none());
    assert!(a.div(I::point(2.).unwrap()).is_none());
    let b = I::point(Real::MAX).unwrap();
    assert!(b.add(b).is_none());
    assert!(b.mul(I::point(2.).unwrap()).is_none());
    let zero = b.sub(b).unwrap();
    assert_eq!([zero.lo, zero.hi], [0., 0.]);
    assert_eq!(b.mul(I::point(0.).unwrap()).unwrap().hi, 0.);
    assert!(I::exact(&(rational(Real::from_bits(1)) / rational(2.))).is_none());
    assert!(
        a.sub(I::point(Real::MIN_POSITIVE.next_up()).unwrap())
            .is_none()
    );
}

fn fixture(offset: Real) -> (Vec<H>, Vec<Uv>, Vec<H>) {
    let net = (0..=2)
        .flat_map(|j| {
            (0..=2).map(move |i| {
                [
                    rational(i as Real / 2.),
                    rational(j as Real / 2.),
                    rational(if i == 2 { 1. } else { 0. } + if j == 2 { 1. } else { 0. }),
                    Rational::one(),
                ]
            })
        })
        .collect::<Vec<_>>();
    // Nonlinear cubic UV inside one quadratic patch; its image has degree 12.
    let uv = [
        [0.4, 0.4],
        [0.4002, 0.4004],
        [0.4007, 0.4007],
        [0.401, 0.401],
    ]
    .map(|p| [rational(p[0]), rational(p[1]), Rational::one()])
    .to_vec();
    let mut budget = Budget(MAX_WORK);
    let uknots = [vec![Rational::zero(); 4], vec![Rational::one(); 4]].concat();
    let mut points = Vec::new();
    for t in [0., 1. / 3., 2. / 3., 1.] {
        let t = rational(t);
        let p = curve::extract(&uknots, 3, 3, &uv, &t, &t, &mut budget).unwrap()[0].clone();
        points.push([
            p[0].clone(),
            p[1].clone(),
            &p[0] * &p[0] + &p[1] * &p[1] + rational(offset),
        ]);
    }
    let weights = [
        [rational(1.), rational(0.), rational(0.), rational(0.)],
        [
            -rational(5.) / rational(6.),
            rational(3.),
            -rational(3.) / rational(2.),
            rational(1.) / rational(3.),
        ],
        [
            rational(1.) / rational(3.),
            -rational(3.) / rational(2.),
            rational(3.),
            -rational(5.) / rational(6.),
        ],
        [rational(0.), rational(0.), rational(0.), rational(1.)],
    ];
    let spatial = weights
        .map(|w| {
            std::array::from_fn(|axis| {
                if axis == 3 {
                    Rational::one()
                } else {
                    points.iter().zip(&w).map(|(p, w)| &p[axis] * w).sum()
                }
            })
        })
        .to_vec();
    (net, uv, spatial)
}

#[test]
fn interval_decisions_agree_with_exact_curved_surface_certificates() {
    let domains = [
        [Rational::zero(), Rational::one()],
        [Rational::zero(), Rational::one()],
    ];
    let mut accepted = 0;
    let mut rejected = 0;
    for offset in [0., 1e-8, 2e-7, 5e-7, 9e-7, 1.1e-6, 2e-6, -2e-6] {
        let (net, uv, spatial) = fixture(offset);
        let result = bound(&net, [2, 2], &domains, &uv, &spatial, 1e-6);
        let controls = net
            .iter()
            .map(|p| {
                crate::WeightedPoint3::try_new(
                    Point3::try_new(
                        scalar(&p[0]).unwrap(),
                        scalar(&p[1]).unwrap(),
                        scalar(&p[2]).unwrap(),
                    )
                    .unwrap(),
                    1.,
                )
                .unwrap()
            })
            .collect();
        let source = NurbsSurface::try_new_rational(
            2,
            2,
            3,
            3,
            controls,
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let mut budget = Budget(MAX_WORK);
        let mut exact = surface::Surface::new(&source, &mut budget)
            .unwrap()
            .unwrap();
        match result {
            Outcome::Within(upper) => {
                assert!(upper <= 1e-6);
                assert!(
                    piece_bound_with_interval(&mut exact, uv, spatial, upper, &mut budget, false)
                        .unwrap()
                        .is_some()
                );
                accepted += 1;
            }
            Outcome::Outside => {
                assert!(
                    piece_bound_with_interval(&mut exact, uv, spatial, 1e-6, &mut budget, false)
                        .unwrap()
                        .is_none()
                );
                rejected += 1;
            }
            Outcome::Inconclusive => {}
        }
    }
    assert!(accepted >= 4);
    assert!(rejected >= 2);
}

#[test]
fn zero_and_subnormal_limits_never_take_the_interval_proof() {
    let (net, uv, spatial) = fixture(0.);
    let domains = [
        [Rational::zero(), Rational::one()],
        [Rational::zero(), Rational::one()],
    ];
    for limit in [0., Real::from_bits(1), Real::MIN_POSITIVE / 2.] {
        assert!(matches!(
            bound(&net, [2, 2], &domains, &uv, &spatial, limit),
            Outcome::Inconclusive
        ));
    }
}

#[test]
fn crossing_box_decisions_agree_with_exact_restriction_on_both_knot_sides() {
    let tolerance = Tolerance::DEFAULT;
    let frame = crate::Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        tolerance,
    )
    .unwrap();
    let source = NurbsSurface::try_cylinder(frame, 2., 0., 4.).unwrap();
    let knot = source.knots_u()[source.degree_u() + 1];
    let point = source.evaluate(knot, 1.).unwrap().to_array();
    let spatial = vec![
        [
            rational(point[0]),
            rational(point[1]),
            rational(point[2]),
            Rational::one()
        ];
        4
    ];
    let mut accepted = 0;
    let mut refined = 0;
    for width in [1e-10, 1e-8, 1e-6, 1e-3, 0.1] {
        let bounds = vec![
            [rational(knot - width), rational(knot + width)],
            [rational(1. - width), rational(1. + width)],
        ];
        let mut budget = Budget(MAX_WORK);
        let mut surface = surface::Surface::new(&source, &mut budget)
            .unwrap()
            .unwrap();
        let result = surface
            .interval_crossing_bound(&bounds, &spatial, 1e-6, &mut budget)
            .unwrap();
        let exact = surface
            .crossing_bound(&bounds, &spatial, 1e-6, &mut budget)
            .unwrap();
        match result {
            Crossing::Within(upper) => {
                assert!(upper <= 1e-6);
                assert!(exact.is_some());
                accepted += 1;
            }
            Crossing::Refine => {
                assert!(exact.is_none());
                refined += 1;
            }
            Crossing::Inconclusive => {}
        }
    }
    assert!(accepted >= 2);
    assert!(refined >= 2);
}

#[test]
fn cubic_sampling_nodes_cannot_hide_a_curved_surface_excursion() {
    let heights = [0., 1. / 18., -5. / 54., 1. / 18., 0.];
    let controls = (0..2)
        .flat_map(|j| {
            (0..5).map(move |i| {
                crate::WeightedPoint3::try_new(
                    Point3::try_new(i as Real / 4., j as Real, heights[i]).unwrap(),
                    1.,
                )
                .unwrap()
            })
        })
        .collect();
    let source = NurbsSurface::try_new_rational(
        4,
        1,
        5,
        2,
        controls,
        vec![0., 0., 0., 0., 0., 1., 1., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let points = [0., 1. / 3., 2. / 3., 1.]
        .map(|x| Point2::try_new(x, 0.5).unwrap())
        .to_vec();
    let uv = NurbsCurve2::try_new(3, points.clone(), vec![0., 0., 0., 0., 1., 1., 1., 1.]).unwrap();
    let spatial = NurbsCurve::try_new(
        3,
        points
            .into_iter()
            .map(|p| Point3::try_new(p.x(), p.y(), 0.).unwrap())
            .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    for t in [0., 1. / 3., 2. / 3., 1.] {
        let p = uv.evaluate(t).unwrap();
        assert!(
            source
                .evaluate(p.x(), p.y())
                .unwrap()
                .distance_to(spatial.evaluate(t).unwrap())
                .unwrap()
                < 1e-6
        );
    }
    assert!(source.evaluate(0.5, 0.5).unwrap().z().abs() > 0.006);
    assert!(
        source
            .parameter_curve_deviation_bound(&uv, &spatial, 1e-6)
            .unwrap()
            .is_none()
    );
}
