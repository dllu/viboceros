#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, Circle3, Frame3, NurbsSurface, Point3, Tolerance, UnitVector3, Vector3,
};
use viboceros_smlib::Shape;
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
#[test]
fn native_surface_apex_retains_curved_base_and_has_analytic_volume() {
    for warped in [false, true] {
        let source = Shape::from_brep(&patch(warped), tol()).unwrap();
        let before = source.to_brep(tol()).unwrap();
        for apex in [p(1., 1.5, 6.), p(1., 1.5, -6.), p(4., -1., 6.)] {
            let b = source
                .extruded_faces_to_point(apex, true, tol())
                .unwrap()
                .to_brep(tol())
                .unwrap();
            assert!(b.is_solid());
            let expected = (2. * apex.z()
                + if warped {
                    0.5 - 0.5 * apex.x() - apex.y() / 3.
                } else {
                    0.
                })
            .abs();
            assert!(
                (b.signed_volume(tol()).unwrap().abs() - expected).abs() < 1e-5,
                "warped={warped} apex={apex:?}"
            );
            for (u, v) in [(0.1, 0.2), (0.4, 0.7), (0.8, 0.9)] {
                let expected = before.faces()[0].surface().evaluate(u, v).unwrap();
                assert!(b.faces().iter().any(|f| {
                    f.surface()
                        .closest_parameters(expected, tol())
                        .is_ok_and(|(a, c)| {
                            f.surface()
                                .evaluate(a, c)
                                .unwrap()
                                .distance_to(expected)
                                .unwrap()
                                < 1e-6
                        })
                }));
            }
            for t in [0.2, 0.5, 0.8] {
                let start = p(1., 0., 0.);
                let expected = start
                    .translated(start.vector_to(apex).unwrap().scaled(t).unwrap())
                    .unwrap();
                assert!(b.faces().iter().any(|f| {
                    f.surface()
                        .closest_parameters(expected, tol())
                        .is_ok_and(|(a, c)| {
                            f.surface()
                                .evaluate(a, c)
                                .unwrap()
                                .distance_to(expected)
                                .unwrap()
                                < 1e-6
                        })
                }));
            }
        }
        assert_eq!(source.to_brep(tol()).unwrap(), before);
    }
}
#[test]
fn native_surface_apex_preserves_rational_holes_and_uncapped_walls() {
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
    let b = source
        .extruded_faces_to_point(p(0., 0., 6.), true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(b.is_solid());
    assert!((b.signed_volume(tol()).unwrap().abs() - 16. * std::f64::consts::PI).abs() < 1e-4);
    let walls = source
        .extruded_faces_to_point(p(0., 0., 6.), false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
#[test]
fn native_surface_apex_declines_coplanar_targets_and_closed_sources() {
    let source = Shape::from_brep(&patch(false), tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    assert!(
        source
            .extruded_faces_to_point(p(1., 1., 0.), true, tol())
            .is_err()
    );
    assert!(
        source
            .extruded_faces_to_point(p(0., 0., 0.), false, tol())
            .is_err()
    );
    assert_eq!(source.to_brep(tol()).unwrap(), before);
    assert!(
        source
            .extruded_faces_to_point(p(1., 1., 6.), true, tol())
            .is_ok()
    );
    let frame = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        tol(),
    )
    .unwrap();
    let source = Shape::from_brep(
        &Brep::try_box(frame, [[0., 1.], [0., 1.], [0., 1.]], tol()).unwrap(),
        tol(),
    )
    .unwrap();
    assert!(
        source
            .extruded_faces_to_point(p(0., 0., 6.), true, tol())
            .is_err()
    );
}

#[test]
fn native_surface_apex_connected_faces_exclude_shared_internal_walls() {
    let carrier = patch(false).faces()[0].surface().clone();
    let grid = Brep::try_surface_grid(&carrier, &[0.5], &[], tol()).unwrap();
    let source = Shape::from_brep(&grid, tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    let b = source
        .extruded_faces_to_point(p(1., 1.5, 6.), true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(b.is_solid());
    assert_eq!(b.faces().len(), 8);
    assert!((b.signed_volume(tol()).unwrap().abs() - 12.).abs() < 1e-5);
    let walls = source
        .extruded_faces_to_point(p(1., 1.5, 6.), false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert_eq!(walls.faces().len(), 6);
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
