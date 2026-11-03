use super::*;

fn cube() -> Brep {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    Brep::try_box(frame, [[0., 8.], [0., 6.], [0., 4.]], Tolerance::DEFAULT).unwrap()
}

#[test]
fn planar_inset_curved_holes_reject_collisions_and_topology_changes() {
    use crate::{Circle3, CurveRef};
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
    let circle = |x, radius| {
        let c = Circle3::try_new(
            Point3::try_new(x, 0., 0.).unwrap(),
            radius,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            tolerance,
        )
        .unwrap();
        CurveRef::Circle(&c).to_nurbs().unwrap()
    };
    let outer = circle(0., 10.);
    let source =
        Brep::try_planar_face_with_holes(&outer, &[circle(-1.5, 1.), circle(1.5, 1.)], tolerance)
            .unwrap();
    let before = source.clone();
    let result = source
        .try_inset_planar_faces(&[0], 0.4, true, tolerance)
        .unwrap();
    assert_eq!(result.faces().len(), 4);
    assert_eq!(
        result.edge_use_counts().iter().filter(|&&n| n == 2).count(),
        3
    );
    for distance in [0.5, 0.6, 9., 10.] {
        assert!(
            source
                .try_inset_planar_faces(&[0], distance, true, tolerance)
                .is_err(),
            "{distance}"
        );
    }
    assert_eq!(source, before);
    let narrow =
        Brep::try_planar_face_with_holes(&circle(0., 5.), &[circle(0., 4.)], tolerance).unwrap();
    assert!(
        narrow
            .try_inset_planar_faces(&[0], 0.2, false, tolerance)
            .is_ok()
    );
    assert!(
        narrow
            .try_inset_planar_faces(&[0], 0.5, false, tolerance)
            .is_err()
    );
    assert!(
        narrow
            .try_inset_planar_faces(&[0], 0.6, false, tolerance)
            .is_err()
    );
}

#[test]
fn planar_inset_hole_collisions_are_detected_in_unsorted_and_vertical_loops() {
    use crate::{Circle3, CurveRef};
    let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
    for vertical in [false, true] {
        let normal = Vector3::try_from(if vertical { [1., 0., 0.] } else { [0., 0., 1.] })
            .unwrap()
            .normalized_nonzero()
            .unwrap();
        let circle = |position, radius| {
            let center = Point3::try_from(if vertical {
                [0., position, 0.]
            } else {
                [position, 0., 0.]
            })
            .unwrap();
            let c = Circle3::try_new(center, radius, normal, tolerance).unwrap();
            CurveRef::Circle(&c).to_nurbs().unwrap()
        };
        let holes = [7., -8., 1., -2.].map(|p| circle(p, 1.));
        let source = Brep::try_planar_face_with_holes(&circle(0., 20.), &holes, tolerance).unwrap();
        let before = source.clone();
        assert_eq!(
            source
                .try_inset_planar_faces(&[0], 0.4, false, tolerance)
                .unwrap()
                .faces()
                .len(),
            6
        );
        for distance in [0.5, 0.6] {
            assert!(
                source
                    .try_inset_planar_faces(&[0], distance, false, tolerance)
                    .is_err()
            );
        }
        assert_eq!(source, before);
    }
}

#[test]
fn planar_inset_keeps_solid_incidence_surfaces_and_source_tables() {
    let source = cube();
    for crease in [false, true] {
        let result = source
            .try_inset_planar_faces(&[0, 1], 0.3, crease, Tolerance::DEFAULT)
            .unwrap();
        assert!(result.is_solid());
        assert_eq!(&result.vertices[..source.vertices.len()], source.vertices());
        assert_eq!(&result.edges[..source.edges.len()], source.edges());
        for (a, b) in source.faces.iter().zip(&result.faces) {
            assert_eq!(a.surface, b.surface);
            assert_eq!(a.reversed, b.reversed);
        }
        assert_eq!(&source.faces[2..], &result.faces[2..6]);
        assert!(
            (source.signed_volume(Tolerance::DEFAULT).unwrap()
                - result.signed_volume(Tolerance::DEFAULT).unwrap())
            .abs()
                < 1e-10
        );
        assert!(
            (source.area(Tolerance::DEFAULT).unwrap() - result.area(Tolerance::DEFAULT).unwrap())
                .abs()
                < 1e-10
        );
    }
}

#[test]
fn planar_inset_rejects_invalid_and_collapsing_requests_without_mutation() {
    let source = cube();
    let before = source.clone();
    for distance in [0., -1., f64::NAN, f64::INFINITY, 1e-16, 3., 5.] {
        assert!(
            source
                .try_inset_planar_faces(&[0], distance, true, Tolerance::DEFAULT)
                .is_err(),
            "{distance}"
        );
    }
    for selected in [vec![], vec![0, 0], vec![6], vec![0, 6]] {
        assert!(
            source
                .try_inset_planar_faces(&selected, 0.3, true, Tolerance::DEFAULT)
                .is_err()
        );
    }
    assert_eq!(source, before);
}
