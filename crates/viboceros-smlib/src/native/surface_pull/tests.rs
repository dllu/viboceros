use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> NurbsSurface {
    NurbsSurface::try_clamped_uniform(
        1,
        1,
        2,
        2,
        vec![
            p(0., 0., 0.),
            p(0., 10., 0.),
            p(10., 0., 0.),
            p(10., 10., 0.),
        ],
    )
    .unwrap()
}
#[test]
fn normal_pull_preserves_inputs_and_certifies_the_complete_lifted_image() {
    let surface = plane();
    let source =
        NurbsCurve::try_new(1, vec![p(1., 5., 3.), p(9., 5., 3.)], vec![2., 2., 7., 7.]).unwrap();
    let before = (source.clone(), surface.clone());
    let curves = pull_curve_to_surface(&source, &surface, Tolerance::DEFAULT).unwrap();
    assert_eq!(curves.len(), 1);
    assert!(curves[0].surface_deviation <= Tolerance::DEFAULT.absolute());
    for point in viboceros_geometry::CurveRef::NurbsCurve(&curves[0].spatial)
        .divide_by_count(32, true, Tolerance::DEFAULT)
        .unwrap()
    {
        assert!(point.z().abs() <= Tolerance::DEFAULT.absolute());
        assert!((point.y() - 5.).abs() <= Tolerance::DEFAULT.absolute());
        assert!(point.x() >= 1. - 1e-8 && point.x() <= 9. + 1e-8);
    }
    assert_eq!((source, surface), before);
}
#[test]
fn pulls_only_the_interval_over_the_finite_surface() {
    let surface = plane();
    let source = NurbsCurve::try_new(
        1,
        vec![p(-2., 5., 1.), p(12., 5., 1.)],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let curves = pull_curve_to_surface(&source, &surface, Tolerance::DEFAULT).unwrap();
    assert_eq!(curves.len(), 1);
    assert!((curves[0].spatial.length(Tolerance::DEFAULT).unwrap() - 10.).abs() < 1e-8);
    for q in curves[0].spatial.control_points() {
        assert!(q.point().x() >= -1e-8 && q.point().x() <= 10. + 1e-8);
    }
}
#[test]
fn a_miss_returns_no_curves() {
    let source = NurbsCurve::try_new(
        1,
        vec![p(-2., -5., 1.), p(12., -5., 1.)],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        pull_curve_to_surface(&source, &plane(), Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn cylinder_normal_pull_uses_the_near_side_and_keeps_height() {
    let surface = NurbsSurface::try_cylinder(
        viboceros_geometry::Frame3::try_from_normal(
            p(0., 0., 0.),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        2.,
        0.,
        5.,
    )
    .unwrap();
    let source = NurbsCurve::try_new(
        1,
        vec![p(3., 0., 0.5), p(3., 0., 4.5)],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let curves = pull_curve_to_surface(&source, &surface, Tolerance::DEFAULT).unwrap();
    assert!(!curves.is_empty());
    for curve in curves {
        for q in viboceros_geometry::CurveRef::NurbsCurve(&curve.spatial)
            .divide_by_count(16, true, Tolerance::DEFAULT)
            .unwrap()
        {
            assert!((q.x() - 2.).abs() < 1e-8 && q.y().abs() < 1e-8);
            assert!(q.z() >= 0.5 - 1e-8 && q.z() <= 4.5 + 1e-8);
        }
    }
}
#[test]
fn normalized_preparation_restores_translated_geometry_and_unequal_uv_domains() {
    let transform = viboceros_geometry::AffineTransform3::try_new(
        [[2., 0., 0.], [0., 2., 0.], [0., 0., 2.]],
        Vector3::try_new(1e6, -2e6, 3e6).unwrap(),
    )
    .unwrap();
    let surface = plane()
        .try_reparameterized(2. ..=7., -8. ..=3.)
        .unwrap()
        .transformed(transform)
        .unwrap();
    let source = NurbsCurve::try_new(1, vec![p(1., 5., 3.), p(9., 5., 3.)], vec![0., 0., 1., 1.])
        .unwrap()
        .transformed(transform)
        .unwrap();
    let result = pull_curve_to_surface(&source, &surface, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 1);
    let domain = result[0].uv.domain();
    for i in 0..=16 {
        let t = *domain.start() + (*domain.end() - *domain.start()) * i as f64 / 16.;
        let uv = result[0].uv.evaluate(t).unwrap();
        let actual = result[0].spatial.evaluate(t).unwrap();
        assert!(
            surface
                .evaluate(uv.x(), uv.y())
                .unwrap()
                .distance_to(actual)
                .unwrap()
                <= Tolerance::DEFAULT.absolute()
        );
        assert!((actual.z() - 3e6).abs() <= Tolerance::DEFAULT.absolute());
    }
}
#[test]
fn a_closed_curve_can_pull_around_the_cylinder() {
    let frame = viboceros_geometry::Frame3::try_from_normal(
        p(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let surface = NurbsSurface::try_cylinder(frame, 2., 0., 5.).unwrap();
    let source =
        viboceros_geometry::Circle3::try_new(p(0., 0., 2.), 3., frame.z_axis(), Tolerance::DEFAULT)
            .unwrap()
            .to_nurbs()
            .unwrap();
    let result = pull_curve_to_surface(&source, &surface, Tolerance::DEFAULT).unwrap();
    assert!(!result.is_empty());
    let mut length = 0.;
    for curve in result {
        length += curve.spatial.length(Tolerance::DEFAULT).unwrap();
        for q in viboceros_geometry::CurveRef::NurbsCurve(&curve.spatial)
            .divide_by_count(32, true, Tolerance::DEFAULT)
            .unwrap()
        {
            assert!((q.x().hypot(q.y()) - 2.).abs() < 1e-8);
            assert!((q.z() - 2.).abs() < 1e-8);
        }
    }
    assert!((length - 4. * std::f64::consts::PI).abs() < 1e-7);
}
#[test]
fn unsupported_preparation_declines_without_mutating_either_input() {
    let source = NurbsCurve::try_new(
        18,
        (0..19).map(|i| p(1. + i as f64 / 3., 5., 3.)).collect(),
        [vec![0.; 19], vec![1.; 19]].concat(),
    )
    .unwrap();
    let surface = plane();
    let before = (source.clone(), surface.clone());
    assert!(pull_curve_to_surface(&source, &surface, Tolerance::DEFAULT).is_err());
    assert_eq!((source, surface), before);
}
#[test]
fn already_on_surface_curves_keep_their_interiors_and_boundary_crossings() {
    for (start, end, length) in [(1., 9., 8.), (-2., 12., 10.)] {
        let source = NurbsCurve::try_new(
            1,
            vec![p(start, 5., 0.), p(end, 5., 0.)],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        let output = pull_curve_to_surface(&source, &plane(), Tolerance::DEFAULT).unwrap();
        assert_eq!(output.len(), 1);
        assert!(
            (output[0].spatial.length(Tolerance::DEFAULT).unwrap() - length).abs()
                <= Tolerance::DEFAULT.absolute()
        );
    }
}
