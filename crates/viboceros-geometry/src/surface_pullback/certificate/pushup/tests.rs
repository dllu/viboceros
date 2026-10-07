use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn q(x: Real, y: Real) -> Point2 {
    Point2::try_new(x, y).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-14, 1e-12).unwrap()
}
fn warped() -> NurbsSurface {
    NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 1.), p(0., 1., 0.)])
        .unwrap()
}
fn path(degree: usize, points: Vec<Point2>, domain: [Real; 2]) -> NurbsCurve2 {
    NurbsCurve2::try_new(
        degree,
        points,
        [vec![domain[0]; degree + 1], vec![domain[1]; degree + 1]].concat(),
    )
    .unwrap()
}
fn check(surface: &NurbsSurface, uv: &NurbsCurve2) -> NurbsCurve {
    let saved = (surface.clone(), uv.clone());
    let (image, bound) = surface
        .try_pushup_curve_certified_with_bound(uv, tol())
        .unwrap();
    assert!(bound >= 0. && bound <= tol().absolute());
    assert_eq!(image.domain(), uv.domain());
    assert!(
        surface
            .parameter_curve_deviation_bound(uv, &image, tol().absolute())
            .unwrap()
            .is_some()
    );
    assert_eq!((&saved.0, &saved.1), (surface, uv));
    image
}

#[test]
fn exact_diagonal_and_nonlinear_warped_images_retain_original_speed() {
    let surface = warped();
    let uv = path(1, vec![q(0., 0.), q(1., 1.)], [2., 5.]);
    let image = check(&surface, &uv);
    assert_eq!(image.degree(), 2);
    assert_eq!(image.control_points().len(), 3);
    assert_eq!(image.evaluate(3.5).unwrap(), p(0.5, 0.5, 0.25));
    let uv = path(2, vec![q(0., 0.), q(0., 0.5), q(1., 1.)], [0., 1.]);
    let image = check(&surface, &uv);
    assert_eq!(image.degree(), 4);
    assert!(
        image
            .evaluate(0.5)
            .unwrap()
            .distance_to(p(0.25, 0.5, 0.125))
            .unwrap()
            < 1e-15
    );
}

#[test]
fn rational_and_negative_gauges_preserve_normalized_parameter_correspondence() {
    for gauge in [1., -3.] {
        let uv = NurbsCurve2::try_new_rational(
            1,
            vec![
                WeightedPoint2::try_new(q(0., 0.), gauge * 2.).unwrap(),
                WeightedPoint2::try_new(q(1., 1.), gauge).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let image = check(&warped(), &uv);
        assert!(
            image
                .evaluate(0.5)
                .unwrap()
                .distance_to(p(1. / 3., 1. / 3., 1. / 9.))
                .unwrap()
                < 1e-15
        );
    }
}

fn crossed() -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(0.3, 0., 0.),
            p(1., 0., 1.),
            p(0., 1., 0.),
            p(0.3, 1., 0.),
            p(1., 1., 1.),
        ],
        vec![0., 0., 0.3, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn exact_rational_linear_tensor_crossings_keep_orientation_and_domain() {
    for points in [
        vec![q(0., 0.25), q(1., 0.25)],
        vec![q(1., 0.25), q(0., 0.25)],
    ] {
        let uv = NurbsCurve2::try_new_rational(
            1,
            points
                .into_iter()
                .zip([2., 1.])
                .map(|(p, w)| WeightedPoint2::try_new(p, w).unwrap())
                .collect(),
            vec![2., 2., 5., 5.],
        )
        .unwrap();
        let image = check(&crossed(), &uv);
        assert_eq!(image.degree(), 1);
        assert_eq!(image.control_points().len(), 4);
    }
}

#[test]
fn nonlinear_nondyadic_tensor_crossings_require_certified_adaptive_fitting() {
    let uv = path(2, vec![q(0., 0.25), q(0., 0.25), q(1., 0.25)], [0., 1.]);
    let image = check(&crossed(), &uv);
    assert_eq!(image.degree(), 3);
    assert!(image.control_points().len() > 4);
    let t = 0.3_f64.sqrt();
    assert!(
        image
            .evaluate(t)
            .unwrap()
            .distance_to(p(0.3, 0.25, 0.))
            .unwrap()
            < 1e-6
    );
}

#[test]
fn original_uv_knots_and_discontinuous_one_sided_limits_are_retained() {
    let uv = NurbsCurve2::try_new(
        1,
        vec![q(0., 0.), q(0.2, 0.2), q(0.8, 0.8), q(1., 1.)],
        vec![0., 0., 0.3, 0.3, 1., 1.],
    )
    .unwrap();
    let image = check(&warped(), &uv);
    assert_eq!(image.control_points().len(), 6);
    assert_eq!(image.evaluate(0.3).unwrap(), p(0.8, 0.8, 0.8 * 0.8));
    assert!(image.evaluate(0.3_f64.next_down()).unwrap().x() < 0.21);
}

#[test]
fn adjacent_and_subnormal_native_domains_do_not_collapse_exact_images() {
    let big = 2_f64.powi(53);
    for domain in [
        [big, big.next_up()],
        [0., Real::from_bits(1)],
        [-Real::MAX, Real::MAX],
    ] {
        let uv = path(2, vec![q(0., 0.), q(0., 0.5), q(1., 1.)], domain);
        check(&warped(), &uv);
    }
}

#[test]
fn composed_spatial_degrees_above_sixteen_are_certified() {
    let controls = (0..2)
        .flat_map(|v| {
            (0..=8).map(move |u| {
                p(
                    u as Real / 8.,
                    v as Real,
                    if u == 8 && v == 1 { 1. } else { 0. },
                )
            })
        })
        .collect();
    let surface = NurbsSurface::try_new(
        8,
        1,
        9,
        2,
        controls,
        [vec![0.; 9], vec![1.; 9]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let uv = path(2, vec![q(0., 0.), q(0., 0.5), q(1., 1.)], [0., 1.]);
    let image = check(&surface, &uv);
    assert_eq!(image.degree(), 18);
}

#[test]
fn unsupported_weights_and_outside_uv_images_are_never_returned() {
    let mixed = NurbsCurve2::try_new_rational(
        1,
        vec![
            WeightedPoint2::try_new(q(0., 0.), 1.).unwrap(),
            WeightedPoint2::try_new(q(1., 1.), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        warped().try_pushup_curve_certified(&mixed, tol()),
        Err(GeometryError::SurfacePushupDidNotConverge { .. })
    ));
    let outside = path(1, vec![q(-0.1, 0.), q(1., 1.)], [0., 1.]);
    assert!(
        warped()
            .try_pushup_curve_certified(&outside, tol())
            .is_err()
    );
}

#[test]
fn constant_image_is_a_valid_nonzero_degree_curve() {
    let uv = path(1, vec![q(0.25, 0.5); 2], [0., 1.]);
    let image = check(&warped(), &uv);
    assert_eq!(image.degree(), 1);
    assert_eq!(image.evaluate(0.).unwrap(), p(0.25, 0.5, 0.125));
    assert_eq!(image.evaluate(0.).unwrap(), image.evaluate(1.).unwrap());
}
