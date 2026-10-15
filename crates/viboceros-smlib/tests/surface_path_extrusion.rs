#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Circle3, NurbsCurve, NurbsSurface, Point3, Tolerance, UnitVector3};
use viboceros_smlib::{KernelCurve, Shape};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap()
}
fn patch(warped: bool) -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_new(
            1,
            1,
            2,
            2,
            vec![
                p(0., 0., 0.),
                p(2., 0., 0.),
                p(0., 3., 0.),
                p(2., 3., if warped { 1. } else { 0. }),
            ],
            vec![0., 0., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap(),
        tol(),
    )
    .unwrap()
}
fn path() -> KernelCurve {
    KernelCurve::from_nurbs_for_modeling(
        &NurbsCurve::try_new(
            2,
            vec![p(10., 20., 30.), p(10., 21., 31.), p(10., 20., 32.)],
            vec![2., 2., 2., 5., 5., 5.],
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn native_surface_path_extrusion_preserves_sources_and_has_correct_spatial_locus() {
    let rail = path();
    let rail_before = rail.to_nurbs().unwrap();
    for warped in [false, true] {
        let source = Shape::from_brep(&patch(warped), tol()).unwrap();
        let before = source.to_brep(tol()).unwrap();
        let result = source
            .extruded_faces_along_curve(&rail, true, tol())
            .unwrap()
            .to_brep(tol())
            .unwrap();
        assert!(result.is_solid());
        assert!((result.signed_volume(tol()).unwrap().abs() - 12.).abs() < 1e-4);
        assert!(result.bounds().min().x().abs() < 1e-6);
        assert!(result.bounds().max().x() < 2. + 1e-6);
        assert!((result.bounds().max().z() - if warped { 3. } else { 2. }).abs() < 1e-6);
        for offset in [
            viboceros_geometry::Vector3::try_new(0., 0., 0.).unwrap(),
            viboceros_geometry::Vector3::try_new(0., 0., 2.).unwrap(),
        ] {
            assert!(
                result.faces().iter().any(|face| (0..8).any(|orientation| {
                    [0., 0.4, 1.].into_iter().all(|u| {
                        [0., 0.6, 1.].into_iter().all(|v| {
                            let expected = before.faces()[0]
                                .surface()
                                .evaluate(u, v)
                                .unwrap()
                                .translated(offset)
                                .unwrap();
                            let (mut a, mut b) = if orientation & 4 == 0 { (u, v) } else { (v, u) };
                            if orientation & 1 != 0 {
                                a = 1. - a;
                            }
                            if orientation & 2 != 0 {
                                b = 1. - b;
                            }
                            let s = face.surface();
                            s.evaluate(s.parameter_at_u(a).unwrap(), s.parameter_at_v(b).unwrap())
                                .is_ok_and(|q| q.distance_to(expected).unwrap() < 1e-6)
                        })
                    })
                })),
                "missing original or translated cap: warped={warped} offset={offset:?}"
            );
        }

        // A straight sweep has the same volume/end caps. These interior wall
        // samples distinguish the quadratic path and fixed profile orientation.
        for t in [0.25, 0.5, 0.75] {
            let expected = p(1., 2. * t * (1. - t), 2. * t);
            assert!(
                result.faces().iter().any(|f| {
                    let surface = f.surface();
                    surface
                        .closest_parameters(expected, tol())
                        .is_ok_and(|(u, v)| {
                            surface
                                .evaluate(u, v)
                                .unwrap()
                                .distance_to(expected)
                                .unwrap()
                                < 1e-6
                        })
                }),
                "missing curved-path wall point {expected:?}"
            );
        }
        assert_eq!(source.to_brep(tol()).unwrap(), before);
    }
    assert_eq!(rail.to_nurbs().unwrap(), rail_before);
}
#[test]
fn native_surface_path_extrusion_retains_rational_holes_and_uncapped_walls() {
    let normal = UnitVector3::try_new(0., 0., 1., tol()).unwrap();
    let circle = |r| {
        Circle3::try_new(p(0., 0., 0.), r, normal, tol())
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let source = Shape::from_brep(
        &Brep::try_planar_face_with_holes(&circle(3.), &[circle(1.)], tol()).unwrap(),
        tol(),
    )
    .unwrap();
    let before = source.to_brep(tol()).unwrap();
    let rail = path();
    let result = source
        .extruded_faces_along_curve(&rail, true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(result.is_solid());
    assert!((result.signed_volume(tol()).unwrap().abs() - 16. * std::f64::consts::PI).abs() < 1e-4);
    let walls = source
        .extruded_faces_along_curve(&rail, false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert!(!walls.faces().is_empty());
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}

#[test]
fn native_surface_path_failures_preserve_reusable_sources() {
    let source = Shape::from_brep(&patch(false), tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    let zero = KernelCurve::from_nurbs_for_modeling(
        &NurbsCurve::try_new(1, vec![p(0., 0., 0.); 2], vec![0., 0., 1., 1.]).unwrap(),
    )
    .unwrap();
    assert!(
        source
            .extruded_faces_along_curve(&zero, true, tol())
            .is_err()
    );
    let closed = KernelCurve::from_nurbs_for_modeling(
        &Circle3::try_new(
            p(0., 0., 0.),
            5.,
            UnitVector3::try_new(1., 0., 0., tol()).unwrap(),
            tol(),
        )
        .unwrap()
        .to_nurbs()
        .unwrap(),
    )
    .unwrap();
    assert!(
        source
            .extruded_faces_along_curve(&closed, false, tol())
            .is_err()
    );
    let tangent = KernelCurve::from_nurbs_for_modeling(
        &NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(1., 0., 0.)], vec![0., 0., 1., 1.]).unwrap(),
    )
    .unwrap();
    assert!(
        source
            .extruded_faces_along_curve(&tangent, true, tol())
            .is_err()
    );
    assert_eq!(source.to_brep(tol()).unwrap(), before);
    assert!(
        source
            .extruded_faces_along_curve(&path(), true, tol())
            .unwrap()
            .to_brep(tol())
            .unwrap()
            .is_solid()
    );
}

#[test]
fn native_surface_path_control_product_limit_declines_before_sweeping() {
    let count = 50_000;
    let points = (0..count).map(|i| p(0., 0., i as f64)).collect::<Vec<_>>();
    let mut knots = vec![0.];
    knots.extend((0..count).map(|i| i as f64));
    knots.push((count - 1) as f64);
    let path =
        KernelCurve::from_nurbs_for_modeling(&NurbsCurve::try_new(1, points, knots).unwrap())
            .unwrap();
    let source = Shape::from_brep(&patch(false), tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    assert!(
        source
            .extruded_faces_along_curve(&path, true, tol())
            .is_err()
    );
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
