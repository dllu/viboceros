#![cfg(feature = "native")]
use viboceros_geometry::{Brep, Frame3, Point3, Tolerance, Vector3};
use viboceros_smlib::Shape;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn tol() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap()
}
fn source() -> Brep {
    let plane = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        tol(),
    )
    .unwrap();
    Brep::try_box(plane, [[0., 10.], [0., 8.], [0., 6.]], tol()).unwrap()
}
fn center(b: &Brep, i: usize) -> Point3 {
    let s = b.faces()[i].surface();
    s.evaluate(
        s.parameter_at_u(0.5).unwrap(),
        s.parameter_at_v(0.5).unwrap(),
    )
    .unwrap()
}
fn expected(b: &Brep, removed: &[usize], t: f64) -> f64 {
    let dims = [10., 8., 6.];
    let mut lo = [t; 3];
    let mut hi = dims.map(|d| d - t);
    for &i in removed {
        let c = center(b, i).to_array();
        for a in 0..3 {
            if c[a].abs() < 1e-8 {
                lo[a] = 0.;
            }
            if (c[a] - dims[a]).abs() < 1e-8 {
                hi[a] = dims[a];
            }
        }
    }
    480. - (0..3).map(|a| hi[a] - lo[a]).product::<f64>()
}
#[test]
fn native_shell_respects_every_original_box_face_and_preserves_source() {
    let b = source();
    let native = Shape::from_brep(&b, tol()).unwrap();
    let before = native.to_brep(tol()).unwrap();
    for i in 0..6 {
        let result = native
            .shelled_faces(&[i, i], 1.)
            .unwrap()
            .to_brep_approximated(tol(), 1e-8)
            .unwrap();
        assert!(result.is_solid());
        let volume = result.signed_volume(tol()).unwrap().abs();
        assert!(
            (volume - expected(&b, &[i], 1.)).abs() < 1e-4,
            "face={i} volume={volume} expected={}",
            expected(&b, &[i], 1.)
        );
    }
    assert_eq!(native.to_brep(tol()).unwrap(), before);
}
#[test]
fn native_shell_opposite_and_adjacent_openings_have_correct_wall_volume() {
    let b = source();
    let top = (0..6)
        .find(|&i| (center(&b, i).z() - 6.).abs() < 1e-8)
        .unwrap();
    let bottom = (0..6).find(|&i| center(&b, i).z().abs() < 1e-8).unwrap();
    let side = (0..6)
        .find(|&i| (center(&b, i).x() - 10.).abs() < 1e-8)
        .unwrap();
    let native = Shape::from_brep(&b, tol()).unwrap();
    for removed in [[top, bottom], [top, side]] {
        let result = native
            .shelled_faces(&removed, 1.)
            .unwrap()
            .to_brep_approximated(tol(), 1e-8)
            .unwrap();
        assert!(result.is_solid());
        assert!(
            (result.signed_volume(tol()).unwrap().abs() - expected(&b, &removed, 1.)).abs() < 1e-4
        );
    }
}
#[test]
fn native_shell_invalid_requests_and_excessive_thickness_preserve_reusable_source() {
    let native = Shape::from_brep(&source(), tol()).unwrap();
    let before = native.to_brep(tol()).unwrap();
    for (faces, t) in [
        (vec![], 1.),
        (vec![6], 1.),
        ((0..6).collect(), 1.),
        (vec![0], 0.),
        (vec![0], -1.),
        (vec![0], f64::NAN),
        (vec![0], 100.),
    ] {
        assert!(
            native.shelled_faces(&faces, t).is_err(),
            "faces={faces:?} t={t}"
        );
    }
    assert_eq!(native.to_brep(tol()).unwrap(), before);
    assert!(native.shelled_faces(&[0], 1.).is_ok());
}

#[test]
fn native_shell_cylinder_cap_produces_curved_wall_volume() {
    let plane = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        tol(),
    )
    .unwrap();
    let b = Brep::try_cylinder(plane, 5., 0., 8., tol()).unwrap();
    let top = b
        .faces()
        .iter()
        .position(|f| {
            f.surface()
                .evaluate(
                    f.surface().parameter_at_u(0.5).unwrap(),
                    f.surface().parameter_at_v(0.5).unwrap(),
                )
                .unwrap()
                .z()
                > 7.999
        })
        .unwrap();
    let result = Shape::from_brep(&b, tol())
        .unwrap()
        .shelled_faces(&[top], 1.)
        .unwrap()
        .to_brep_approximated(tol(), 1e-8)
        .unwrap();
    assert!(result.is_solid());
    assert!((result.signed_volume(tol()).unwrap().abs() - 88. * std::f64::consts::PI).abs() < 1e-3);
}
#[test]
fn native_shell_face_reordering_and_disconnected_walls() {
    let b = source();
    let mut faces = b.faces().to_vec();
    faces.reverse();
    let b = Brep::try_new(b.vertices().to_vec(), b.edges().to_vec(), faces, tol()).unwrap();
    let removed = (0..6)
        .filter(|&i| {
            let c = center(&b, i);
            c.z().abs() < 1e-8
                || (c.z() - 6.).abs() < 1e-8
                || c.y().abs() < 1e-8
                || (c.y() - 8.).abs() < 1e-8
        })
        .collect::<Vec<_>>();
    let result = Shape::from_brep(&b, tol())
        .unwrap()
        .shelled_faces(&removed, 1.)
        .unwrap()
        .to_brep_approximated(tol(), 1e-8)
        .unwrap();
    assert!(result.is_solid());
    assert!((result.signed_volume(tol()).unwrap().abs() - 96.).abs() < 1e-4);
    assert_eq!(result.edge_connected_face_components().len(), 2);
}

#[test]
fn native_shell_default_tolerance_excessive_thickness_recovers() {
    let tolerance = Tolerance::default();
    let native = Shape::from_brep(&source(), tolerance).unwrap();
    assert!(native.shelled_faces(&[0], 100.).is_err());
    assert!(native.shelled_faces(&[0], 1.).is_ok());
}
