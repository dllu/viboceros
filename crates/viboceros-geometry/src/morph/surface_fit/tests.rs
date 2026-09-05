use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn unit_patch() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(1.0, 1.0, 0.0),
        ]
        .map(|p| WeightedPoint3::try_new(p, 1.0).unwrap())
        .to_vec(),
        vec![-7.0, -7.0, 13.0, 13.0],
        vec![2.0, 2.0, 6.0, 6.0],
    )
    .unwrap()
}

struct Cubic;
impl PointMorph for Cubic {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(
            p.x(),
            p.y(),
            p.z() + p.x().powi(2) + p.x() * p.y() + p.y().powi(3),
        )
    }
}

struct Quartic;
impl PointMorph for Quartic {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(p.x(), p.y(), p.z() + p.x().powi(4))
    }
}

fn check_image(
    source: &NurbsSurface,
    fitted: &NurbsSurface,
    morph: &impl PointMorph,
    epsilon: Real,
) {
    assert_eq!(fitted.domain_u(), source.domain_u());
    assert_eq!(fitted.domain_v(), source.domain_v());
    let fraction = |i| match i {
        0 => 0.0,
        32 => 1.0,
        _ => (i as Real - 0.3819660112501051) / 32.0,
    };
    for j in 0..=32 {
        for i in 0..=32 {
            let u = source.parameter_at_u(fraction(i)).unwrap();
            let v = source.parameter_at_v(fraction(j)).unwrap();
            let expected = morph.morph_point(source.evaluate(u, v).unwrap()).unwrap();
            let actual = fitted.evaluate(u, v).unwrap();
            assert!(
                actual.distance_to(expected).unwrap() <= epsilon,
                "u={u} v={v}: {actual:?} != {expected:?}"
            );
        }
    }
}

#[test]
fn cubic_surface_image_is_not_the_bilinear_image_of_its_controls() {
    let source = unit_patch();
    let fitted = Cubic
        .morph_nurbs_surface(&source, Tolerance::DEFAULT)
        .unwrap();
    check_image(&source, &fitted, &Cubic, 2e-12);
}

#[test]
fn quartic_surface_image_requires_refinement_beyond_mapped_controls() {
    let source = unit_patch();
    let fitted = Quartic
        .morph_nurbs_surface(&source, Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap())
        .unwrap();
    check_image(&source, &fitted, &Quartic, 1e-7);
    assert!(fitted.control_point_count_u() > 4);
    assert_eq!(fitted.control_point_count_v(), 4);
}

#[test]
fn two_nonlinear_directions_are_refined_independently() {
    struct Both;
    impl PointMorph for Both {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), p.y(), p.z() + p.x().powi(4) + p.y().powi(4))
        }
    }
    let source = unit_patch();
    let fitted = Both
        .morph_nurbs_surface(&source, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap())
        .unwrap();
    assert!(fitted.control_point_count_u() > 4);
    assert!(fitted.control_point_count_v() > 4);
    check_image(&source, &fitted, &Both, 1e-6);
}

#[test]
fn affine_map_keeps_the_rational_control_net() {
    struct Affine;
    impl PointMorph for Affine {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(3.0 * p.y() - 2.0, p.x() + 4.0, 2.0 * p.z())
        }
    }
    let patch = unit_patch();
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        patch
            .control_points()
            .iter()
            .enumerate()
            .map(|(i, cp)| WeightedPoint3::try_new(cp.point(), (i + 1) as Real).unwrap())
            .collect(),
        patch.knots_u().to_vec(),
        patch.knots_v().to_vec(),
    )
    .unwrap();
    let fitted = Affine
        .morph_nurbs_surface(&source, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(fitted, mapped_controls(&Affine, &source).unwrap());
    check_image(&source, &fitted, &Affine, 3e-15);
}

#[test]
fn morph_need_not_be_defined_at_off_surface_controls() {
    struct SurfaceOnly;
    impl PointMorph for SurfaceOnly {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            if p.y() > 1.0 + 1e-12 {
                return Err(GeometryError::Degenerate {
                    context: "map only defined on surface",
                });
            }
            Point3::try_new(p.x(), p.y().powi(2), p.z())
        }
    }
    let source = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        [0.0, 1.0]
            .into_iter()
            .flat_map(|z| {
                [p(0.0, 0.0, z), p(0.5, 2.0, z), p(1.0, 0.0, z)]
                    .map(|p| WeightedPoint3::try_new(p, 1.0).unwrap())
            })
            .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    assert!(mapped_controls(&SurfaceOnly, &source).is_err());
    let fitted = SurfaceOnly
        .morph_nurbs_surface(&source, Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap())
        .unwrap();
    check_image(&source, &fitted, &SurfaceOnly, 1e-6);
}

#[test]
fn fitting_limits_return_errors_instead_of_inaccurate_surfaces() {
    let source = unit_patch();
    assert!(matches!(
        fit(&Quartic, &source, Tolerance::DEFAULT, 4, 10_000),
        Err(GeometryError::SurfaceMorphDidNotConverge { deviation, tolerance, .. })
            if deviation > tolerance
    ));
    assert!(matches!(
        fit(&Quartic, &source, Tolerance::DEFAULT, 256, 7),
        Err(GeometryError::TooManyMorphSurfaceSamples { maximum: 7 })
    ));
    assert!(matches!(
        fit(&Quartic, &source, Tolerance::DEFAULT, 1, 10_000),
        Err(GeometryError::TooManyMorphSurfaceControlPoints { maximum: 1 })
    ));
}

#[test]
fn nonuniform_validation_detects_regular_grid_aliasing() {
    struct Ripple;
    impl PointMorph for Ripple {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), p.y(), (48.0 * std::f64::consts::PI * p.x()).sin())
        }
    }
    assert!(matches!(
        fit(&Ripple, &unit_patch(), Tolerance::DEFAULT, 4, 10_000),
        Err(GeometryError::SurfaceMorphDidNotConverge { deviation, .. }) if deviation > 0.1
    ));
}

#[test]
fn tensor_interpolation_retains_constant_large_world_coordinates_exactly() {
    let source = unit_patch();
    let breaks = [
        seed_breaks(1, source.knots_u(), source.domain_u()),
        seed_breaks(1, source.knots_v(), source.domain_v()),
    ];
    let expected = p(1e12, -2e12, 3e12);
    let fitted = tensor::interpolate(&mut |_, _| Ok(expected), &breaks).unwrap();
    assert!(
        fitted
            .control_points()
            .iter()
            .all(|cp| cp.point() == expected)
    );
}

#[test]
fn periodic_source_keeps_its_seam_and_native_parameters() {
    let profile = NurbsCurve::try_control_point_curve_with_closure(
        2,
        vec![
            p(-0.3, 0.0, 0.0),
            p(0.0, 0.3, 0.0),
            p(0.3, 0.0, 0.0),
            p(0.0, -0.3, 0.0),
        ],
        crate::ControlPointCurveClosure::Smooth,
    )
    .unwrap();
    let source = NurbsSurface::try_extruded_curve(
        &profile,
        Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
    )
    .unwrap();
    assert!(source.is_periodic_u());
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-10).unwrap();
    let fitted = Cubic.morph_nurbs_surface(&source, tolerance).unwrap();
    check_image(&source, &fitted, &Cubic, 1e-6);
    for j in 0..=32 {
        let v = source.parameter_at_v(j as Real / 32.0).unwrap();
        let start = fitted.evaluate(*source.domain_u().start(), v).unwrap();
        let end = fitted.evaluate(*source.domain_u().end(), v).unwrap();
        assert!(start.distance_to(end).unwrap() <= 1e-14);
    }
}

#[test]
fn full_order_surface_jumps_keep_four_independent_morphed_limits() {
    use crate::ParameterSide::{Left, Right};
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        4,
        [0.0, 2.0, 5.0, 7.0]
            .into_iter()
            .flat_map(|y| {
                [0.0, 1.0, 3.0, 4.0]
                    .map(move |x| WeightedPoint3::try_new(p(x, y, 0.0), 1.0).unwrap())
            })
            .collect(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let fitted = Cubic
        .morph_nurbs_surface(&source, Tolerance::DEFAULT)
        .unwrap();
    for su in [Left, Right] {
        for sv in [Left, Right] {
            let expected = Cubic
                .morph_point(source.evaluate_on_sides(1.0, 1.0, su, sv).unwrap())
                .unwrap();
            assert_eq!(
                fitted.evaluate_on_sides(1.0, 1.0, su, sv).unwrap(),
                expected
            );
        }
    }
    check_image(&source, &fitted, &Cubic, 2e-12);
}
