#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, BrepWireCage, Circle3, CurvePreviewCage, GeometryError, NurbsCurve, NurbsSurface,
    ParameterSide, Point3, PointMorph, SurfacePreviewCage, Tolerance, UnitVector3,
};
use viboceros_smlib::{morph_preview_operations, prepare_morph_preview_curve};

struct Cube;
impl PointMorph for Cube {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(p.x(), p.y(), p.z() + p.x().powi(3))
    }
}
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn compare(a: &NurbsCurve, b: &NurbsCurve) {
    assert_eq!(a.degree(), b.degree());
    assert_eq!(a.knots(), b.knots());
    assert_eq!(a.control_points().len(), b.control_points().len());
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert!((a.weight() - b.weight()).abs() < 1e-12);
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-8);
    }
    for i in 0..=32 {
        let t = a.parameter_at(i as f64 / 32.).unwrap();
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(b.evaluate(t).unwrap())
                .unwrap()
                < 1e-8
        );
    }
}

#[test]
fn native_curve_preparation_supports_independent_full_order_cubic_pieces() {
    let source = NurbsCurve::try_new(
        1,
        vec![p(0., 0., 0.), p(1., 0., 0.), p(4., 0., 0.), p(5., 0., 0.)],
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    assert!(CurvePreviewCage::try_new(&source).is_err());
    let raised = prepare_morph_preview_curve(&source).unwrap();
    assert_eq!(raised.degree(), 3);
    assert_eq!(raised.control_points().len(), 8);
    let cage = CurvePreviewCage::try_new_with_operations(&source, Some(morph_preview_operations()))
        .unwrap();
    // The cage retains its producer even when the point map uses default policy.
    let curve = cage.morphed_curve(&Cube, false).unwrap();
    for (control, (x, z)) in curve.control_points().iter().zip([
        (0., 0.),
        (1. / 3., 0.),
        (2. / 3., 0.),
        (1., 1.),
        (4., 64.),
        (13. / 3., 80.),
        (14. / 3., 100.),
        (5., 125.),
    ]) {
        assert!((control.point().x() - x).abs() < 1e-11);
        assert!((control.point().z() - z).abs() < 1e-10);
    }
    for side in [ParameterSide::Left, ParameterSide::Right] {
        assert!(
            curve
                .evaluate_on_side(0.5, side)
                .unwrap()
                .distance_to(
                    Cube.morph_point(source.evaluate_on_side(0.5, side).unwrap())
                        .unwrap()
                )
                .unwrap()
                < 1e-10
        );
    }
    let preserved = cage.morphed_curve(&Cube, true).unwrap();
    assert_eq!(preserved.degree(), 1);
    assert_eq!(source, before);
}

#[test]
fn native_surface_preparation_keeps_separated_tensor_limits() {
    let source = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        (0..2)
            .flat_map(|j| {
                [0., 1., 4., 5.]
                    .into_iter()
                    .map(move |x| p(x, j as f64, 0.))
            })
            .collect(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let before = source.clone();
    assert!(SurfacePreviewCage::try_new(&source).is_err());
    let surface =
        SurfacePreviewCage::try_new_with_operations(&source, Some(morph_preview_operations()))
            .unwrap()
            .morphed_surface(&Cube, false)
            .unwrap();
    assert_eq!((surface.degree_u(), surface.degree_v()), (3, 3));
    assert_eq!(
        (
            surface.control_point_count_u(),
            surface.control_point_count_v()
        ),
        (8, 4)
    );
    for side in [ParameterSide::Left, ParameterSide::Right] {
        for v in [0., 0.17, 0.73, 1.] {
            assert!(
                surface
                    .evaluate_on_sides(0.5, v, side, ParameterSide::Right)
                    .unwrap()
                    .distance_to(
                        Cube.morph_point(
                            source
                                .evaluate_on_sides(0.5, v, side, ParameterSide::Right)
                                .unwrap()
                        )
                        .unwrap()
                    )
                    .unwrap()
                    < 1e-10
            );
        }
    }
    assert_eq!(source, before);
}

#[test]
fn native_brep_preview_extracts_and_trims_wires_around_rational_holes() {
    let tolerance = Tolerance::DEFAULT;
    let normal = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let outer = Circle3::try_new(p(0., 0., 5.), 5., normal, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let hole = Circle3::try_new(p(0., 0., 5.), 2., normal, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let source = Brep::try_planar_face_with_holes(&outer, &[hole], tolerance).unwrap();
    let before = source.clone();
    let native = BrepWireCage::try_new_with_operations(
        &source,
        1,
        tolerance,
        Some(morph_preview_operations()),
    )
    .unwrap();
    let portable = BrepWireCage::try_new(&source, 1, tolerance).unwrap();
    let a = native.morphed_wires(&Cube, false).unwrap();
    let b = portable.morphed_wires(&Cube, false).unwrap();
    assert_eq!(a.len(), 6);
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(&b) {
        compare(a, b);
    }
    for curve in a.iter().skip(2) {
        for i in 0..=32 {
            let q = curve
                .evaluate(curve.parameter_at(i as f64 / 32.).unwrap())
                .unwrap();
            assert!(q.x().hypot(q.y()) >= 2. - 1e-8);
        }
    }
    assert_eq!(source, before);
}

#[test]
fn higher_degree_direct_cages_keep_the_existing_portable_extraction_path() {
    let source = NurbsSurface::try_new(
        18,
        3,
        19,
        4,
        (0..4)
            .flat_map(|j| (0..19).map(move |i| p(i as f64 / 18., j as f64 / 3., 0.)))
            .collect(),
        [vec![0.; 19], vec![1.; 19]].concat(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let source = Brep::try_surface_face(source, Tolerance::DEFAULT).unwrap();
    let before = source.clone();
    let a = BrepWireCage::try_new_with_operations(
        &source,
        1,
        Tolerance::DEFAULT,
        Some(morph_preview_operations()),
    )
    .unwrap()
    .morphed_wires(&Cube, false)
    .unwrap();
    let b = BrepWireCage::try_new(&source, 1, Tolerance::DEFAULT)
        .unwrap()
        .morphed_wires(&Cube, false)
        .unwrap();
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(&b) {
        compare(a, b);
    }
    assert_eq!(source, before);
}

#[test]
fn native_trim_chart_retains_subnormal_interior_boundaries() {
    let source = NurbsCurve::try_new(
        3,
        vec![
            p(-5., 0., -125.),
            p(-5. / 3., 0., 125.),
            p(5. / 3., 0., -125.),
            p(5., 0., 125.),
        ],
        vec![-5., -5., -5., -5., 5., 5., 5., 5.],
    )
    .unwrap();
    let before = source.clone();
    for start in [f64::from_bits(1), -f64::from_bits(1), 1e-320] {
        let trimmed = viboceros_smlib::trim_curve(&source, start..=2.).unwrap();
        assert_eq!(trimmed.domain(), start..=2.);
        assert_eq!(trimmed.knots()[0], start);
        for i in 0..=32 {
            let t = trimmed.parameter_at(i as f64 / 32.).unwrap();
            assert!(
                trimmed
                    .evaluate(t)
                    .unwrap()
                    .distance_to(source.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
    assert_eq!(source, before);
}

#[test]
fn native_interpolation_chart_retains_subnormal_source_knots() {
    let start = f64::from_bits(1);
    let knots = [start, start, 10., 10.];
    let parameters = [start, 10.];
    let fixed_rows = [1, 1];
    let targets = [0., 0., 1e-280, 1., 1., 2., 2e-280, 1.];
    let result =
        viboceros_smlib::morph_interpolation(&viboceros_geometry::FixedInterpolationSystem {
            system: viboceros_geometry::NurbsInterpolationSystem {
                degree: 1,
                knots: &knots,
                parameters: &parameters,
                periodic: false,
                columns: 4,
                targets: &targets,
            },
            fixed_rows: &fixed_rows,
        })
        .unwrap();
    assert_eq!(result, targets);
    assert_eq!(knots, [start, start, 10., 10.]);
}
