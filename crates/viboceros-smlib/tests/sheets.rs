#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Point3, Tolerance, Vector3};
use viboceros_smlib::{Shape, Solid};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 3., 0.),
            point(0., 3., 0.),
        ])
        .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn sheet_round_trip_preserves_naked_edges_and_face_orientation() {
    for reversed in [false, true] {
        let mut input = plane();
        if reversed {
            input.reverse_orientation();
        }
        let before = input.clone();
        assert!(Solid::from_brep(&input, Tolerance::DEFAULT).is_err());
        let shape = Shape::from_brep(&input, Tolerance::DEFAULT).unwrap();
        let output = shape.to_brep(Tolerance::DEFAULT).unwrap();
        assert!(!output.is_solid());
        assert_eq!(output.edge_use_counts(), vec![1; 4]);
        let surface = output.faces()[0].surface();
        let normal = surface
            .normal_at(
                surface.parameter_at_u(0.5).unwrap(),
                surface.parameter_at_v(0.5).unwrap(),
            )
            .unwrap();
        assert_eq!(normal.as_vector().z() < 0., reversed);
        assert!((output.area(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-10);
        assert_eq!(input, before);
    }
}

#[test]
fn native_planar_offsets_and_walls_preserve_source_geometry() {
    let input = plane();
    let before = input.clone();
    let shape = Shape::from_brep(&input, Tolerance::DEFAULT).unwrap();
    for distance in [0.5, -0.5] {
        for solid in [false, true] {
            let output = shape
                .offset(distance, solid)
                .unwrap()
                .to_brep_approximated(Tolerance::DEFAULT, 1e-8)
                .unwrap();
            assert_eq!(output.is_solid(), solid);
            if solid {
                assert!((output.signed_volume(Tolerance::DEFAULT).unwrap() - 6.).abs() < 1e-8);
                assert!(output.vertices().iter().all(|v| {
                    (distance.min(0.) - 1e-8..=distance.max(0.) + 1e-8).contains(&v.point().z())
                }));
            } else {
                assert!(
                    output
                        .vertices()
                        .iter()
                        .all(|v| (v.point().z() - distance).abs() < 1e-8)
                );
                assert!((output.area(Tolerance::DEFAULT).unwrap() - 12.).abs() < 1e-8);
            }
        }
    }
    assert_eq!(
        shape
            .to_brep(Tolerance::DEFAULT)
            .unwrap()
            .area(Tolerance::DEFAULT)
            .unwrap(),
        12.
    );
    assert_eq!(input, before);
    for distance in [0., f64::NAN, f64::INFINITY] {
        assert!(shape.offset(distance, false).is_err());
    }
}

#[test]
fn periodic_sheet_round_trip_retains_seams_and_boundary_curves() {
    let frame = Frame3::try_from_normal(
        point(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let input = Brep::try_surface_face(
        NurbsSurface::try_cylinder(frame, 2., 0., 3.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let output = Shape::from_brep(&input, Tolerance::DEFAULT)
        .unwrap()
        .to_brep(Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(output.faces().len(), 1);
    assert!(!output.is_solid());
    assert!(output.edge_use_counts().contains(&2));
    assert!((output.area(Tolerance::DEFAULT).unwrap() - 12. * std::f64::consts::PI).abs() < 1e-8);
}
