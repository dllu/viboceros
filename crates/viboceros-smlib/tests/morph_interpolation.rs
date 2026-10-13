#![cfg(feature = "native")]
use viboceros_geometry::{
    CurvePreviewCage, GeometryError, MorphInterpolationSolver, NurbsCurve, NurbsSurface,
    ParameterSide, Point3, PointMorph, SurfacePreviewCage, Tolerance, WeightedPoint3,
};

struct Lift(bool);
impl PointMorph for Lift {
    fn interpolation_solver(&self) -> Option<MorphInterpolationSolver> {
        self.0.then_some(viboceros_smlib::morph_interpolation)
    }
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(
            p.x(),
            p.y(),
            p.z() + p.x().powi(3) + 0.2 * p.y().powi(3) + 0.3 * p.x() * p.y(),
        )
    }
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap()
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn compare_curve(a: &NurbsCurve, b: &NurbsCurve) {
    assert_eq!(a.degree(), b.degree());
    assert_eq!(a.knots(), b.knots());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!((a.weight() - b.weight()).abs() < 1e-9);
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-8);
    }
}
fn compare_surface(a: &NurbsSurface, b: &NurbsSurface) {
    assert_eq!((a.degree_u(), a.degree_v()), (b.degree_u(), b.degree_v()));
    assert_eq!(a.knots_u(), b.knots_u());
    assert_eq!(a.knots_v(), b.knots_v());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!((a.weight() - b.weight()).abs() < 1e-9);
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-8);
    }
}

#[test]
fn native_curve_morphs_keep_rational_composition_and_independent_full_order_limits() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            p(0., 0., 0.),
            p(0.4, 0.7, 0.),
            p(0.8, -0.2, 0.),
            p(1., 0.3, 0.),
        ]
        .into_iter()
        .zip([1., 0.65, 1.2, 0.85])
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .collect(),
        vec![0., 0., 0., 0.35, 1., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    for degree in [2, 3] {
        let source = source.try_change_degree(degree, false).unwrap();
        let a = Lift(true).morph_nurbs_curve(&source, tolerance()).unwrap();
        let b = Lift(false).morph_nurbs_curve(&source, tolerance()).unwrap();
        compare_curve(&a, &b);
        assert_eq!(a.degree(), 3 * degree);
        for i in 0..=100 {
            let t = source.parameter_at(i as f64 / 100.).unwrap();
            assert!(
                a.evaluate(t)
                    .unwrap()
                    .distance_to(Lift(true).morph_point(source.evaluate(t).unwrap()).unwrap())
                    .unwrap()
                    < 1e-7
            );
        }
    }
    assert_eq!(source, before);
    let source = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 0.), p(1., 0., 0.), p(4., 1., 0.), p(5., 1., 0.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let a = Lift(true).morph_nurbs_curve(&source, tolerance()).unwrap();
    compare_curve(
        &a,
        &Lift(false).morph_nurbs_curve(&source, tolerance()).unwrap(),
    );
    assert_eq!(a.control_points().len(), 8);
    for side in [ParameterSide::Left, ParameterSide::Right] {
        assert!(
            a.evaluate_on_side(0.5, side)
                .unwrap()
                .distance_to(
                    Lift(true)
                        .morph_point(source.evaluate_on_side(0.5, side).unwrap())
                        .unwrap()
                )
                .unwrap()
                < 1e-10
        );
    }
}

#[test]
fn native_tensor_morphs_keep_rational_weights_and_direct_uv_images() {
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.), p(1., 1., 0.)]
            .into_iter()
            .zip([1., 0.6, 1.4, 0.85])
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    for degree in [1, 2, 3] {
        let source = source.try_change_degree(degree, degree, false).unwrap();
        let a = Lift(true)
            .morph_nurbs_surface(&source, tolerance())
            .unwrap();
        let b = Lift(false)
            .morph_nurbs_surface(&source, tolerance())
            .unwrap();
        compare_surface(&a, &b);
        assert_eq!((a.degree_u(), a.degree_v()), (3 * degree, 3 * degree));
        for j in 0..=16 {
            for i in 0..=16 {
                let (u, v) = (i as f64 / 16., j as f64 / 16.);
                assert!(
                    a.evaluate(u, v)
                        .unwrap()
                        .distance_to(
                            Lift(true)
                                .morph_point(source.evaluate(u, v).unwrap())
                                .unwrap()
                        )
                        .unwrap()
                        < 1e-7
                );
            }
        }
    }
    assert_eq!(source, before);
}

#[test]
fn native_prepared_curve_and_surface_cages_match_portable_without_source_edits() {
    let curve =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(1., 0.4, 0.)], vec![0., 0., 1., 1.]).unwrap();
    let surface =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
            .unwrap();
    let before = (curve.clone(), surface.clone());
    let solver = Some(viboceros_smlib::morph_interpolation as MorphInterpolationSolver);
    let a = CurvePreviewCage::try_new_with_solver(&curve, solver).unwrap();
    let b = CurvePreviewCage::try_new(&curve).unwrap();
    compare_curve(
        &a.morphed_curve(&Lift(true), false).unwrap(),
        &b.morphed_curve(&Lift(false), false).unwrap(),
    );
    let a = SurfacePreviewCage::try_new_with_solver(&surface, solver).unwrap();
    let b = SurfacePreviewCage::try_new(&surface).unwrap();
    compare_surface(
        &a.morphed_surface(&Lift(true), false).unwrap(),
        &b.morphed_surface(&Lift(false), false).unwrap(),
    );
    assert_eq!((curve, surface), before);
}

#[test]
fn native_preview_interpolation_retains_nonconstant_tiny_channels() {
    struct Tiny;
    impl PointMorph for Tiny {
        fn interpolation_solver(&self) -> Option<MorphInterpolationSolver> {
            Some(viboceros_smlib::morph_interpolation)
        }
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), p.y(), p.z() + 1e-280 * p.x().powi(3))
        }
    }
    let source =
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(1., 0., 0.)], vec![0., 0., 1., 1.]).unwrap();
    let a = CurvePreviewCage::try_new_with_solver(&source, Tiny.interpolation_solver())
        .unwrap()
        .morphed_curve(&Tiny, false)
        .unwrap();
    assert_eq!(a.degree(), 3);
    for (control, expected) in a.control_points().iter().zip([0., 0., 0., 1.]) {
        assert!((control.point().z() / 1e-280 - expected).abs() < 1e-12);
    }
}
