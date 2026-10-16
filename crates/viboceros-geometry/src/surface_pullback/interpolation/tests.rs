use super::*;

#[test]
fn isocurve_proposals_preserve_source_domains_and_decline_misleading_hints() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let surface = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    p(
                        [0., 0.5, 1.25][i],
                        [0., 0.5, 1.25][j],
                        i as Real * j as Real / 4.,
                    )
                })
            })
            .collect(),
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let tolerance = Tolerance::try_new(1e-6, 1e-14, 1e-10).unwrap();
    for along_u in [true, false] {
        let endpoints = if along_u {
            [[0., 0.2], [1., 0.2]]
        } else {
            [[0.2, 0.], [0.2, 1.]]
        }
        .map(|p| Point2::try_new(p[0], p[1]).unwrap());
        let source = if along_u {
            surface.isocurve_u(0.2)
        } else {
            surface.isocurve_v(0.2)
        }
        .unwrap()
        .try_reparameterized(-3. ..=7.)
        .unwrap();
        let (uv, bound) = fit_on_isocurve(&surface, &source, endpoints, tolerance)
            .unwrap()
            .unwrap();
        assert_eq!(uv.domain(), source.domain());
        assert!(bound <= tolerance.absolute());
        assert!(uv.control_points().iter().all(|p| if along_u {
            p.point().y() == 0.2
        } else {
            p.point().x() == 0.2
        }));
        let middle = if along_u {
            Point2::try_new(0.5, 0.45)
        } else {
            Point2::try_new(0.45, 0.5)
        }
        .unwrap();
        let curved = NurbsCurve2::try_new(
            2,
            vec![endpoints[0], middle, endpoints[1]],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let source = surface
            .try_pushup_curve_certified(&curved, tolerance)
            .unwrap();
        assert!(fit_on_isocurve(&surface, &source, endpoints, tolerance).is_err());
        let (actual, bound) = surface
            .try_pullback_curve_certified_with_bound(&source, Some(endpoints), tolerance)
            .unwrap();
        let middle = actual.evaluate(actual.parameter_at(0.5).unwrap()).unwrap();
        assert!(if along_u {
            middle.y() > 0.3
        } else {
            middle.x() > 0.3
        });
        assert!(bound <= tolerance.absolute());
        let mut controls = source.control_points().to_vec();
        let i = controls.len() / 2;
        let point = controls[i].point();
        controls[i] = crate::WeightedPoint3::try_new(
            p(point.x(), point.y(), point.z() + 0.01),
            controls[i].weight(),
        )
        .unwrap();
        let wrong =
            NurbsCurve::try_new_rational(source.degree(), controls, source.knots().to_vec())
                .unwrap();
        assert!(
            surface
                .try_pullback_curve_certified_with_bound(&wrong, Some(endpoints), tolerance)
                .is_err()
        );
    }
}

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-14, 1e-12).unwrap()
}
fn graph() -> NurbsSurface {
    NurbsSurface::try_new(
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
    .unwrap()
}
fn quadratic_path(domain: [Real; 2]) -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(1. / 3., 1. / 3., 0.),
            p(1., 1., 1.),
        ],
        [vec![domain[0]; 4], vec![domain[1]; 4]].concat(),
    )
    .unwrap()
}
fn check(
    surface: &NurbsSurface,
    source: &NurbsCurve,
    endpoints: Option<[Point2; 2]>,
) -> NurbsCurve2 {
    let saved = (surface.clone(), source.clone());
    let (uv, bound) = surface
        .try_pullback_curve_certified_with_bound(source, endpoints, tolerance())
        .unwrap();
    assert!(bound.is_finite() && bound >= 0. && bound <= tolerance().absolute());
    assert_eq!(uv.domain(), source.domain());
    assert!(
        surface
            .parameter_curve_deviation_bound(&uv, source, tolerance().absolute())
            .unwrap()
            .is_some()
    );
    if let Some(points) = endpoints {
        assert_eq!([uv.start_point().unwrap(), uv.end_point().unwrap()], points);
    }
    assert_eq!((&saved.0, &saved.1), (surface, source));
    uv
}

#[test]
fn nonlinear_paths_with_singular_endpoints_fit_in_both_directions() {
    let surface = graph();
    for source in [
        quadratic_path([2., 5.]),
        quadratic_path([2., 5.]).reversed().unwrap(),
    ] {
        let uv = check(&surface, &source, None);
        assert_eq!(uv.degree(), 3);
        assert!(surface.try_pullback_curve(&source, tolerance()).is_ok());
        let mut points = [
            Point2::try_new(0., 0.).unwrap(),
            Point2::try_new(1., 1.).unwrap(),
        ];
        if *source.domain().start() < 0. {
            points.reverse();
        }
        check(&surface, &source, Some(points));
    }
}

#[test]
fn fractional_fitting_retains_adjacent_and_subnormal_source_domains() {
    let big = 2_f64.powi(53);
    for domain in [[big, big.next_up()], [0., Real::from_bits(1)]] {
        check(&graph(), &quadratic_path(domain), None);
    }
}

#[test]
fn two_singular_endpoints_recover_a_nonlinear_missing_coordinate() {
    let surface = NurbsSurface::try_new(
        1,
        2,
        2,
        3,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(0., 2., 0.5),
            p(2., 2., 0.5),
            p(0., 0., 1.),
            p(0., 0., 1.),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let source = NurbsCurve::try_new(
        4,
        vec![
            p(0., 0., 0.),
            p(0.25, 1., 0.25),
            p(1. / 3., 4. / 3., 0.5),
            p(0.75, 1., 0.75),
            p(0., 0., 1.),
        ],
        [vec![2.; 5], vec![5.; 5]].concat(),
    )
    .unwrap();
    for chart in [surface.clone(), surface.try_swapped_uv().unwrap()] {
        for curve in [source.clone(), source.reversed().unwrap()] {
            check(&chart, &curve, None);
        }
    }
}

#[test]
fn nonlinear_cubic_uv_paths_do_not_require_jacobian_inversion_at_the_start() {
    let source = NurbsCurve::try_new(
        4,
        vec![
            p(0., 0., 0.),
            p(0., 0., 0.),
            p(1. / 6., 0., 0.),
            p(0.5, 0.25, 0.),
            p(1., 1., 1.),
        ],
        [vec![0.; 5], vec![1.; 5]].concat(),
    )
    .unwrap();
    check(&graph(), &source, None);
}

#[test]
fn nonpolynomial_uv_paths_refine_until_the_entire_image_is_certified() {
    let source =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(1., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let uv = check(&graph(), &source, None);
    assert!(uv.control_points().len() > 4);
}
