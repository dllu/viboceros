use super::*;
fn cube() -> Brep {
    Brep::try_box(
        Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        [[0., 2.], [0., 3.], [0., 5.]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn cutting_local_fans_separates_only_required_vertices_and_preserves_all_surfaces_and_curves() {
    let source = cube();
    let original = source.clone();
    for (edges, vertices, count) in [
        (vec![0], 8, 13),
        (vec![0, 3], 9, 14),
        (vec![0, 3, 8], 10, 15),
    ] {
        let parts = source.try_unjoin_edges(&edges, Tolerance::DEFAULT).unwrap();
        assert_eq!(parts.len(), 1);
        let output = &parts[0];
        assert_eq!(output.vertices.len(), vertices);
        assert_eq!(output.edges.len(), count);
        assert!(!output.is_closed());
        for (face, before) in output.faces.iter().zip(&source.faces) {
            assert_eq!(face.surface, before.surface);
            assert_eq!(face.reversed, before.reversed);
            for (trim, original) in face
                .loops
                .iter()
                .flat_map(|l| &l.trims)
                .zip(before.loops.iter().flat_map(|l| &l.trims))
            {
                assert_eq!(trim.curve, original.curve);
                assert_eq!(trim.tolerance, original.tolerance);
                assert_eq!(trim.iso, original.iso);
            }
        }
        for edge in &output.edges {
            assert!(source.edges.iter().any(
                |original| original.curve == edge.curve && original.tolerance == edge.tolerance
            ));
        }
    }
    assert_eq!(source, original);
}
#[test]
fn cap_and_all_edge_cuts_produce_disjoint_face_components_and_duplicates_are_idempotent() {
    let source = cube();
    let parts = source
        .try_unjoin_edges(&[0, 1, 2, 3], Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(
        parts.iter().map(|p| p.faces.len()).collect::<Vec<_>>(),
        [1, 5]
    );
    let parts = source
        .try_unjoin_edges(&(0..12).collect::<Vec<_>>(), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(parts.len(), 6);
    assert!(
        parts
            .iter()
            .all(|p| p.faces.len() == 1 && p.vertices.len() == 4 && p.edges.len() == 4)
    );
    assert_eq!(
        source
            .try_unjoin_edges(&[0, 0, 3, 0], Tolerance::DEFAULT)
            .unwrap(),
        source
            .try_unjoin_edges(&[0, 3], Tolerance::DEFAULT)
            .unwrap()
    );
    assert!(
        source
            .try_unjoin_edges(&[], Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    assert!(
        source
            .try_unjoin_edges(&[0, 12], Tolerance::DEFAULT)
            .is_err()
    );
    assert!(
        source
            .try_unjoin_edges(&vec![0; 100001], Tolerance::DEFAULT)
            .is_err()
    );
}

#[test]
fn split_natural_boundaries_cover_the_domain_but_gaps_overlaps_and_off_boundary_trims_do_not() {
    let source = cube()
        .try_split_edges_at_parameters(&[(0, vec![0.5, 1.5])], Tolerance::DEFAULT)
        .unwrap();
    let face = &source.faces[0];
    assert!(face.is_untrimmed(Tolerance::DEFAULT).unwrap());
    let index = face.loops[0]
        .trims
        .iter()
        .position(|trim| {
            let start = trim.curve.start_point().unwrap();
            let end = trim.curve.end_point().unwrap();
            let axis = if matches!(trim.iso, SurfaceIso::South | SurfaceIso::North) {
                0
            } else {
                1
            };
            start.to_array()[axis] != end.to_array()[axis]
                && face.loops[0]
                    .trims
                    .iter()
                    .filter(|other| other.iso == trim.iso)
                    .count()
                    > 1
        })
        .unwrap();
    let original = &face.loops[0].trims[index];
    let a = original.curve.start_point().unwrap().to_array();
    let b = original.curve.end_point().unwrap().to_array();
    let axis = if matches!(original.iso, SurfaceIso::South | SurfaceIso::North) {
        0
    } else {
        1
    };
    for mode in ["gap", "overlap", "off-boundary"] {
        let mut bad = face.clone();
        let mut start = a;
        let mut end = b;
        if mode == "gap" {
            start[axis] = a[axis] + 0.1 * (b[axis] - a[axis]);
        } else if mode == "overlap" {
            start[axis] = a[axis] - 0.1 * (b[axis] - a[axis]);
        } else {
            start[1 - axis] += 0.1;
            end[1 - axis] += 0.1;
        }
        bad.loops[0].trims[index].curve = NurbsCurve2::try_line(
            Point2::try_from(start).unwrap(),
            Point2::try_from(end).unwrap(),
        )
        .unwrap();
        assert!(!bad.is_untrimmed(Tolerance::DEFAULT).unwrap(), "{mode}");
    }
}
