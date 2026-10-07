use super::*;

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
