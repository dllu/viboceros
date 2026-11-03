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
