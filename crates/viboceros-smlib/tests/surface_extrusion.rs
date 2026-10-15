#![cfg(feature = "native")]
use viboceros_geometry::{
    Brep, Circle3, Frame3, NurbsSurface, Point3, Tolerance, UnitVector3, Vector3,
};
use viboceros_smlib::Shape;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::try_new(x, y, z).unwrap()
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
fn native_surface_sweep_retains_curved_caps_and_has_analytic_volume() {
    for warped in [false, true] {
        let source = Shape::from_brep(&patch(warped), tol()).unwrap();
        let before = source.to_brep(tol()).unwrap();
        let normal = source.single_face_plane_normal(tol()).unwrap();
        assert_eq!(normal.is_none(), warped);
        if let Some(n) = normal {
            assert!((n.z().abs() - 1.).abs() < 1e-12);
        }
        for displacement in [v(0., 0., 2.), v(0., 0., -2.), v(1., 0., 2.)] {
            let result = source
                .extruded_faces(displacement, true, tol())
                .unwrap()
                .to_brep(tol())
                .unwrap();
            assert!(result.is_solid());
            assert_eq!(result.faces().len(), 6);
            for offset in [v(0., 0., 0.), displacement] {
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
                                let (mut a, mut b) =
                                    if orientation & 4 == 0 { (u, v) } else { (v, u) };
                                if orientation & 1 != 0 {
                                    a = 1. - a;
                                }
                                if orientation & 2 != 0 {
                                    b = 1. - b;
                                }
                                let s = face.surface();
                                s.evaluate(
                                    s.parameter_at_u(a).unwrap(),
                                    s.parameter_at_v(b).unwrap(),
                                )
                                .is_ok_and(|q| q.distance_to(expected).unwrap() < 1e-6)
                            })
                        })
                    })),
                    "missing original or translated cap: warped={warped} offset={offset:?}"
                );
            }

            let expected = if warped {
                12. - 1.5 * displacement.x()
            } else {
                12.
            };
            assert!(
                (result.signed_volume(tol()).unwrap().abs() - expected).abs() < 1e-5,
                "{warped} {displacement:?}"
            );
        }
        assert_eq!(source.to_brep(tol()).unwrap(), before);
    }
}
#[test]
fn native_surface_sweep_preserves_rational_trim_holes_and_uncapped_walls() {
    let normal = UnitVector3::try_new(0., 0., 1., tol()).unwrap();
    let circle = |r| {
        Circle3::try_new(p(0., 0., 0.), r, normal, tol())
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let ring = Brep::try_planar_face_with_holes(&circle(3.), &[circle(1.)], tol()).unwrap();
    let source = Shape::from_brep(&ring, tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    let solid = source
        .extruded_faces(v(0., 0., 2.), true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(solid.is_solid());
    assert!((solid.signed_volume(tol()).unwrap().abs() - 16. * std::f64::consts::PI).abs() < 1e-4);
    let walls = source
        .extruded_faces(v(0., 0., 2.), false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert_eq!(walls.faces().len(), 2);
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
#[test]
fn native_surface_sweep_declines_invalid_displacements_and_closed_sources() {
    let source = Shape::from_brep(&patch(false), tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    assert!(source.extruded_faces(v(0., 0., 0.), true, tol()).is_err());
    assert!(source.extruded_faces(v(1., 0., 0.), true, tol()).is_err());
    assert_eq!(source.to_brep(tol()).unwrap(), before);
    let frame =
        Frame3::try_from_directions(p(0., 0., 0.), v(1., 0., 0.), v(0., 1., 0.), tol()).unwrap();
    let solid = Shape::from_brep(
        &Brep::try_box(frame, [[0., 1.], [0., 1.], [0., 1.]], tol()).unwrap(),
        tol(),
    )
    .unwrap();
    assert!(solid.extruded_faces(v(0., 0., 2.), true, tol()).is_err());
}

#[test]
fn native_surface_sweep_of_connected_faces_does_not_create_an_interior_wall() {
    let surface = patch(false).faces()[0].surface().clone();
    let grid = Brep::try_surface_grid(&surface, &[0.5], &[], tol()).unwrap();
    let source = Shape::from_brep(&grid, tol()).unwrap();
    let before = source.to_brep(tol()).unwrap();
    let result = source
        .extruded_faces(v(0., 0., 2.), true, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(result.is_solid());
    assert_eq!(result.faces().len(), 10);
    assert!((result.signed_volume(tol()).unwrap().abs() - 12.).abs() < 1e-5);
    let walls = source
        .extruded_faces(v(0., 0., 2.), false, tol())
        .unwrap()
        .to_brep(tol())
        .unwrap();
    assert!(!walls.is_solid());
    assert_eq!(walls.faces().len(), 6);
    assert_eq!(source.to_brep(tol()).unwrap(), before);
}
