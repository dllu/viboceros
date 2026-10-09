use super::*;
use crate::{WeightedPoint2, WeightedPoint3};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn line(a: Point3, b: Point3) -> NurbsCurve {
    NurbsCurve::try_new(1, vec![a, b], vec![0., 0., 1., 1.]).unwrap()
}
fn warped() -> NurbsSurface {
    NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 1.), p(0., 1., 0.)])
        .unwrap()
}
fn diagonal(gauge: Real, domain: [Real; 2]) -> NurbsCurve2 {
    NurbsCurve2::try_new_rational(
        1,
        [[0., 0.], [1., 1.]]
            .map(|v| WeightedPoint2::try_new(Point2::try_new(v[0], v[1]).unwrap(), gauge).unwrap())
            .into(),
        vec![domain[0], domain[0], domain[1], domain[1]],
    )
    .unwrap()
}
fn quadratic(gauge: Real, domain: [Real; 2], offset: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            p(0., 0., offset),
            p(0.5, 0.5, offset),
            p(1., 1., 1. + offset),
        ]
        .map(|v| WeightedPoint3::try_new(v, gauge).unwrap())
        .into(),
        [vec![domain[0]; 3], vec![domain[1]; 3]].concat(),
    )
    .unwrap()
}

#[test]
fn prepared_surface_bounds_match_independent_proofs_and_recover_after_rejection() {
    let source = warped();
    let before = source.clone();
    let mut prepared = source.prepare_surface_curve_bounds().unwrap().unwrap();
    for _ in 0..3 {
        for gauge in [1., -2., 1e-280] {
            let uv = diagonal(gauge, [1e9, 1e9 + 4.]);
            for offset in [0., 1. / 1024., 1.] {
                let spatial = quadratic(-gauge, [-1e300, 1e300], offset);
                for limit in [0., 1e-6, 1. / 1024.] {
                    assert_eq!(
                        prepared.bound(&uv, &spatial, limit).unwrap(),
                        source
                            .parameter_curve_deviation_bound(&uv, &spatial, limit)
                            .unwrap()
                    );
                }
            }
        }
        let uv = diagonal(1., [0., 1.]);
        let spatial = quadratic(1., [0., 1.], 0.);
        assert!(matches!(
            prepared.bound(&uv, &spatial, Real::NAN),
            Err(GeometryError::InvalidTolerance)
        ));
        assert_eq!(prepared.bound(&uv, &spatial, 0.).unwrap(), Some(0.));
    }
    assert_eq!(source, before);
}

#[test]
fn exact_warped_images_ignore_weight_gauges_and_curve_domain_scales() {
    for gauge in [1., -2., 1e-280, -1e280] {
        let uv = diagonal(gauge, [1e9, 1e9 + 4.]);
        let spatial = quadratic(-gauge, [-1e300, 1e300], 0.);
        let s = warped();
        let before = (s.clone(), uv.clone(), spatial.clone());
        assert_eq!(
            s.parameter_curve_deviation_bound(&uv, &spatial, 0.)
                .unwrap(),
            Some(0.)
        );
        assert_eq!((s, uv, spatial), before);
    }
    let offset = 1. / 1024.;
    let s = warped();
    let uv = diagonal(1., [0., 1.]);
    let spatial = quadratic(1., [0., 1.], offset);
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &spatial, offset)
            .unwrap(),
        Some(offset)
    );
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &spatial, offset.next_down())
            .unwrap(),
        None
    );
}

#[test]
fn rational_surface_and_rational_parameter_speed_cancel_exactly() {
    for gauge in [1., -2., 1e-280, 1e280] {
        let s = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            [p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)]
                .into_iter()
                .zip([1., 2., 1., 2.])
                .map(|(p, w)| WeightedPoint3::try_new(p, w * gauge).unwrap())
                .collect(),
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let uv = NurbsCurve2::try_new_rational(
            1,
            [[0., 0.25], [1., 0.25]]
                .into_iter()
                .zip([2., 1.])
                .map(|(p, w)| {
                    WeightedPoint2::try_new(Point2::try_new(p[0], p[1]).unwrap(), w * gauge)
                        .unwrap()
                })
                .collect(),
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let edge = line(p(0., 0.25, 0.), p(1., 0.25, 0.));
        assert_eq!(
            s.parameter_curve_deviation_bound(&uv, &edge, 0.).unwrap(),
            Some(0.)
        );
    }
}

#[test]
fn whole_span_hulls_refine_an_interior_gap_instead_of_checking_only_endpoints() {
    let s = warped();
    let uv = NurbsCurve2::try_line(
        Point2::try_new(0., 0.).unwrap(),
        Point2::try_new(1., 0.).unwrap(),
    )
    .unwrap();
    let edge = NurbsCurve::try_new(
        2,
        vec![p(0., 0., 0.), p(0.5, 0., 0.01), p(1., 0., 0.)],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &edge, 0.004)
            .unwrap(),
        None
    );
    let bound = s
        .parameter_curve_deviation_bound(&uv, &edge, 0.006)
        .unwrap()
        .unwrap();
    assert!((0.005..=0.006).contains(&bound));
}

#[test]
fn exact_knot_alignment_and_nonrepresentable_crossings_keep_continuous_bounds() {
    for knot in [0.5, 0.3] {
        let s = NurbsSurface::try_new(
            1,
            1,
            3,
            2,
            [0., 1.]
                .into_iter()
                .flat_map(|v| [0., knot, 1.].map(|u| p(u, v, 0.)))
                .collect(),
            vec![0., 0., knot, 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let uv = NurbsCurve2::try_line(
            Point2::try_new(0., 0.25).unwrap(),
            Point2::try_new(1., 0.25).unwrap(),
        )
        .unwrap();
        let edge = line(p(0., 0.25, 0.), p(1., 0.25, 0.));
        let bound = s
            .parameter_curve_deviation_bound(&uv, &edge, 1e-9)
            .unwrap()
            .unwrap();
        assert!((0. ..=1e-9).contains(&bound));
        if knot == 0.5 {
            assert_eq!(
                s.parameter_curve_deviation_bound(&uv, &edge, 0.).unwrap(),
                Some(0.)
            );
        }
    }
    let edge = quadratic(1., [-4., 8.], 0.);
    let refined = edge.try_insert_knot(0.5, 1).unwrap();
    assert_eq!(
        warped()
            .parameter_curve_deviation_bound(&diagonal(1., [0., 1.]), &refined, 1e-14)
            .unwrap()
            .unwrap(),
        0.
    );
}

#[test]
fn a_polynomial_excursion_between_every_sample_station_is_rejected() {
    // z(u,v)=v*prod(u-i/16), i=1..16. At u=v=k/16 all 17
    // uniform stations vanish, although the intervals contain large excursions.
    let choose = |n: usize, k: usize| {
        (0..k.min(n - k)).fold(1_u64, |v, i| v * (n - i) as u64 / (i + 1) as u64)
    };
    let mut power = vec![Rational::one()];
    for i in 1..=16 {
        let root = rational(i as Real / 16.);
        let mut next = vec![Rational::zero(); power.len() + 1];
        for (j, c) in power.iter().enumerate() {
            next[j] -= &root * c;
            next[j + 1] += c;
        }
        power = next;
    }
    let z = (0..=16)
        .map(|i| {
            let b: Rational = (0..=i)
                .map(|j| &power[j] * Rational::new(choose(i, j).into(), choose(16, j).into()))
                .sum();
            scalar(&(b * rational(1e10))).unwrap()
        })
        .collect::<Vec<_>>();
    let controls = (0..2)
        .flat_map(|v| {
            z.iter()
                .enumerate()
                .map(move |(i, &z)| p(i as Real / 16., v as Real, z * v as Real))
        })
        .collect();
    let s = NurbsSurface::try_new(
        16,
        1,
        17,
        2,
        controls,
        [vec![0.; 17], vec![1.; 17]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let uv = diagonal(1., [0., 1.]);
    let edge = line(p(0., 0., 0.), p(1., 1., 0.));
    for i in 0..=16 {
        let t = i as Real / 16.;
        assert!(
            s.evaluate(t, t)
                .unwrap()
                .distance_to(edge.evaluate(t).unwrap())
                .unwrap()
                < 1e-6
        );
    }
    assert!(s.evaluate(1. / 32., 1. / 32.).unwrap().z().abs() > 0.01);
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &edge, 1e-6).unwrap(),
        None
    );
}

#[test]
fn uncertified_domains_signs_limits_and_resource_exhaustion_remain_explicit() {
    let s = warped();
    let uv = diagonal(1., [0., 1.]);
    let edge = quadratic(1., [0., 1.], 0.);
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert!(
            s.parameter_curve_deviation_bound(&uv, &edge, limit)
                .is_err()
        );
    }
    let mixed = NurbsCurve2::try_new_rational(
        1,
        vec![
            WeightedPoint2::try_new(Point2::try_new(0., 0.).unwrap(), 1.).unwrap(),
            WeightedPoint2::try_new(Point2::try_new(1., 1.).unwrap(), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        s.parameter_curve_deviation_bound(&mixed, &edge, 1.)
            .unwrap(),
        None
    );
    let outside = NurbsCurve2::try_line(
        Point2::try_new(-1., 0.).unwrap(),
        Point2::try_new(1., 1.).unwrap(),
    )
    .unwrap();
    assert_eq!(
        s.parameter_curve_deviation_bound(&outside, &edge, 1.)
            .unwrap(),
        None
    );
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv.reversed().unwrap(), &edge, 0.1)
            .unwrap(),
        None
    );
    let mut certificate = PullbackCertificate::with_degree(&s, &edge, uv.degree())
        .unwrap()
        .unwrap();
    certificate.budget = Budget(0);
    assert!(matches!(
        certificate.curve(&uv, 1.),
        Err(GeometryError::SurfaceCurveCertificateWorkLimit)
    ));
    let huge = Rational::from_integer(num_bigint::BigInt::one() << 8193);
    assert!(matches!(
        Budget(MAX_WORK).check(&huge),
        Err(GeometryError::SurfaceCurveCertificateWorkLimit)
    ));
}

#[test]
fn exact_rational_linear_crossings_split_both_axes_at_non_binary_fractions() {
    // u(t)=v(t)=2t/(1+t). Tensor knots at u=1/2 and v=1/4
    // cross at exact t=1/3 and t=1/7, neither a binary64 parameter.
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        3,
        [0., 0.25, 1.]
            .into_iter()
            .enumerate()
            .flat_map(|(j, v)| {
                [0., 0.5, 1.]
                    .into_iter()
                    .enumerate()
                    .map(move |(i, u)| p(u, v, (i == 2) as u8 as Real + (j == 2) as u8 as Real))
            })
            .collect(),
        vec![0., 0., 0.5, 1., 1.],
        vec![0., 0., 0.25, 1., 1.],
    )
    .unwrap();
    let spatial = NurbsCurve::try_new_rational(
        1,
        [
            ([0., 0., 0.], 1.),
            ([0.25, 0.25, 0.], 8. / 7.),
            ([0.5, 0.5, 1. / 3.], 4. / 3.),
            ([1., 1., 2.], 2.),
        ]
        .into_iter()
        .map(|(xyz, w)| WeightedPoint3::try_new(p(xyz[0], xyz[1], xyz[2]), w).unwrap())
        .collect(),
        vec![0., 0., 1. / 7., 1. / 3., 1., 1.],
    )
    .unwrap();
    let uv = NurbsCurve2::try_new_rational(
        1,
        vec![
            WeightedPoint2::try_new(Point2::try_new(0., 0.).unwrap(), 1.).unwrap(),
            WeightedPoint2::try_new(Point2::try_new(1., 1.).unwrap(), 2.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    for (chart, parameters) in [
        (surface.clone(), uv.clone()),
        (surface.try_swapped_uv().unwrap(), uv),
    ] {
        for (parameters, source) in [
            (parameters.clone(), spatial.clone()),
            (parameters.reversed().unwrap(), spatial.reversed().unwrap()),
        ] {
            let bound = chart
                .parameter_curve_deviation_bound(&parameters, &source, 1e-12)
                .unwrap()
                .unwrap();
            assert!(bound < 1e-14, "{bound}");
            assert_eq!(
                chart
                    .parameter_curve_deviation_bound(&parameters, &source, 0.)
                    .unwrap(),
                None
            );
        }
    }
}

#[test]
fn exact_isocurve_restrictions_handle_unequal_uv_weights_and_reversed_axes() {
    // S(u,v)=(u²,v,uv), v=1/2, u(t)=2t/(1+t). The rational
    // quadratic spatial controls are independently composed in Bernstein form.
    let surface = NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(0., 1., 0.),
            p(0., 1., 0.5),
            p(1., 1., 1.),
        ],
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let source = NurbsCurve::try_new_rational(
        2,
        [
            ([0., 0.5, 0.], 1.),
            ([0., 0.5, 0.25], 2.),
            ([1., 0.5, 0.5], 4.),
        ]
        .into_iter()
        .map(|(xyz, w)| WeightedPoint3::try_new(p(xyz[0], xyz[1], xyz[2]), w).unwrap())
        .collect(),
        vec![2., 2., 2., 5., 5., 5.],
    )
    .unwrap();
    for swapped in [false, true] {
        let chart = if swapped {
            surface.try_swapped_uv().unwrap()
        } else {
            surface.clone()
        };
        let uv = NurbsCurve2::try_new_rational(
            1,
            [([0., 0.5], 1.), ([1., 0.5], 2.)]
                .into_iter()
                .map(|(xy, w)| {
                    WeightedPoint2::try_new(
                        Point2::try_new(xy[swapped as usize], xy[1 - swapped as usize]).unwrap(),
                        w,
                    )
                    .unwrap()
                })
                .collect(),
            vec![-7., -7., 19., 19.],
        )
        .unwrap();
        for (uv, curve) in [
            (uv.clone(), source.clone()),
            (uv.reversed().unwrap(), source.reversed().unwrap()),
        ] {
            assert_eq!(
                chart
                    .parameter_curve_deviation_bound(&uv, &curve, 0.)
                    .unwrap(),
                Some(0.)
            );
        }
    }
}

#[test]
fn certified_pullback_accepts_exact_and_fitted_regular_parameterizations() {
    let s = warped();
    let edge = quadratic(1., [0., 1.], 0.);
    let uv = s
        .try_pullback_curve_certified(&edge, Tolerance::DEFAULT)
        .unwrap();
    assert!(
        s.parameter_curve_deviation_bound(&uv, &edge, 1e-9)
            .unwrap()
            .is_some()
    );
    let s = NurbsSurface::try_bilinear([
        p(0., 0., 0.),
        p(10., 0., 0.),
        p(8., 10., 0.),
        p(0., 10., 0.),
    ])
    .unwrap();
    let edge = NurbsCurve::try_new(
        3,
        vec![
            p(0., 2., 0.),
            p(3.2, 20. / 3., 0.),
            p(82. / 15., 8., 0.),
            p(8.8, 6., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(
        s.try_pullback_exact_curve(&edge, Tolerance::DEFAULT)
            .is_err()
    );
    let uv = s
        .try_pullback_curve_certified(&edge, Tolerance::DEFAULT)
        .unwrap();
    let bound = s
        .parameter_curve_deviation_bound(&uv, &edge, 1e-9)
        .unwrap()
        .unwrap();
    assert!(bound <= 1e-9);
    for i in 0..=256 {
        let t = i as Real / 256.;
        let uv = uv.evaluate(t).unwrap();
        assert!(
            s.evaluate(uv.x(), uv.y())
                .unwrap()
                .distance_to(edge.evaluate(t).unwrap())
                .unwrap()
                <= bound + 1e-13
        );
    }
}

#[test]
fn unclamped_source_knots_and_extreme_binary64_distances_do_not_change_proofs() {
    let s = warped().try_reparameterized(0. ..=2., 0. ..=1.).unwrap();
    let uv = NurbsCurve2::try_line(
        Point2::try_new(0., 0.).unwrap(),
        Point2::try_new(2., 0.).unwrap(),
    )
    .unwrap();
    let edge = NurbsCurve::try_new(
        2,
        [-0.5, 0.5, 1.5, 2.5].map(|x| p(x / 2., 0., 0.)).into(),
        vec![-2., -1., 0., 1., 2., 3., 4.],
    )
    .unwrap();
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &edge, 0.).unwrap(),
        Some(0.)
    );
    let s = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            p(-Real::MAX, 0., 0.),
            p(Real::MAX, 0., 0.),
            p(-Real::MAX, 1., 0.),
            p(Real::MAX, 1., 0.),
        ],
        vec![-Real::MAX, -Real::MAX, Real::MAX, Real::MAX],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let uv = NurbsCurve2::try_line(
        Point2::try_new(-Real::MAX, 0.25).unwrap(),
        Point2::try_new(Real::MAX, 0.25).unwrap(),
    )
    .unwrap();
    let edge = line(p(-Real::MAX, 0.25, 0.), p(Real::MAX, 0.25, 0.));
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &edge, 0.).unwrap(),
        Some(0.)
    );
    let tiny = Real::from_bits(1);
    let s = warped();
    let uv = NurbsCurve2::try_line(
        Point2::try_new(0., 0.).unwrap(),
        Point2::try_new(1., 0.).unwrap(),
    )
    .unwrap();
    let edge = line(p(0., 0., tiny), p(1., 0., tiny));
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &edge, tiny).unwrap(),
        Some(tiny)
    );
    assert_eq!(
        s.parameter_curve_deviation_bound(&uv, &edge, 0.).unwrap(),
        None
    );
}

#[test]
fn maximum_image_degree_uses_exact_products_beyond_machine_integer_binomials() {
    // Tensor degrees 8+8 composed with degree 4 produce degree 64; comparison
    // against degree 16 needs degree-80 binomials that do not fit in u64.
    let surface = NurbsSurface::try_new(
        8,
        8,
        9,
        9,
        (0..=8)
            .flat_map(|j| (0..=8).map(move |i| p(i as Real / 8., j as Real / 8., 0.)))
            .collect(),
        [vec![0.; 9], vec![1.; 9]].concat(),
        [vec![0.; 9], vec![1.; 9]].concat(),
    )
    .unwrap();
    let uv = NurbsCurve2::try_new(
        4,
        (0..=4)
            .map(|i| Point2::try_new(i as Real / 4., 0.5).unwrap())
            .collect(),
        [vec![0.; 5], vec![1.; 5]].concat(),
    )
    .unwrap();
    let spatial = NurbsCurve::try_new(
        16,
        (0..=16).map(|i| p(i as Real / 16., 0.5, 0.)).collect(),
        [vec![0.; 17], vec![1.; 17]].concat(),
    )
    .unwrap();
    assert_eq!(
        surface
            .parameter_curve_deviation_bound(&uv, &spatial, 0.)
            .unwrap(),
        Some(0.)
    );
    let unsupported = NurbsCurve2::try_new(
        5,
        (0..=5)
            .map(|i| Point2::try_new(i as Real / 5., 0.5).unwrap())
            .collect(),
        [vec![0.; 6], vec![1.; 6]].concat(),
    )
    .unwrap();
    assert_eq!(
        surface
            .parameter_curve_deviation_bound(&unsupported, &spatial, 1.)
            .unwrap(),
        None
    );
}
