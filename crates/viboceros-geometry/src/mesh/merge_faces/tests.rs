use super::*;

#[test]
fn invalid_late_merge_components_and_nonedge_pairs_preserve_the_source() {
    let source = TriangleMesh::try_new_faces(
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
            Point3::try_new(2., 2., 0.).unwrap(),
            Point3::try_new(0., 2., 0.).unwrap(),
        ],
        vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let before = source.clone();
    for selected in [
        vec![MeshMergeComponent::Face(0), MeshMergeComponent::Face(2)],
        vec![MeshMergeComponent::Vertex(4)],
        vec![MeshMergeComponent::Edge([1, 3])],
        vec![MeshMergeComponent::Edge([0, 0])],
        vec![MeshMergeComponent::Edge([0, usize::MAX])],
    ] {
        assert!(source.merge_face_components(&selected).is_err());
        assert_eq!(source, before);
    }
    assert_eq!(
        source.merge_face_components(&[]).unwrap(),
        (source.clone(), 0)
    );
}

#[test]
fn single_boundary_merge_restores_coplanar_hole_support_in_each_thread() {
    let points = (0..4)
        .flat_map(|y| (0..4).map(move |x| Point3::try_new(x as f64, y as f64, 0.).unwrap()))
        .collect();
    let faces = (0..3)
        .flat_map(|y| {
            (0..3).filter_map(move |x| {
                if (x, y) == (1, 1) {
                    None
                } else {
                    Some(MeshFace::Quad([
                        4 * y + x,
                        4 * y + x + 1,
                        4 * y + x + 5,
                        4 * y + x + 4,
                    ]))
                }
            })
        })
        .collect();
    let ring = TriangleMesh::try_new_faces(points, faces, Tolerance::DEFAULT).unwrap();
    let selected = (0..8).map(MeshMergeComponent::Face).collect::<Vec<_>>();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let ring = &ring;
            let selected = &selected;
            scope.spawn(move || {
                for _ in 0..16 {
                    let (unchanged, count) = ring.merge_face_components(selected).unwrap();
                    assert_eq!(count, 0);
                    assert_eq!(unchanged, *ring);
                    let (merged, count) = ring.merge_coplanar_faces(Tolerance::DEFAULT).unwrap();
                    assert_eq!(count, 7);
                    assert_eq!(merged.ngons().len(), 1);
                    assert_eq!(merged.ngons()[0].faces().len(), 8);
                }
            });
        }
    });
}

#[test]
fn changed_merges_heal_or_reject_degenerate_records_without_mutating_the_source() {
    let source = TriangleMesh::try_from_face_records(
        [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
            [10., 0., 0.],
            [11., 0., 0.],
            [12., 0., 0.],
        ]
        .into_iter()
        .map(|p| Point3::try_new(p[0], p[1], p[2]).unwrap())
        .collect(),
        vec![
            MeshFace::Triangle([0, 1, 2]),
            MeshFace::Triangle([0, 2, 3]),
            MeshFace::Triangle([4, 5, 6]),
        ],
    )
    .unwrap();
    assert!(
        source
            .validate_face_geometry(Tolerance::MESH_VALIDATION)
            .is_err()
    );
    let before = source.clone();
    if let Ok((merged, removed)) =
        source.merge_face_components(&[MeshMergeComponent::Face(0), MeshMergeComponent::Face(1)])
    {
        if merged != source {
            merged
                .validate_face_geometry(Tolerance::MESH_VALIDATION)
                .unwrap();
        } else {
            assert_eq!(removed, 0);
        }
    }
    assert_eq!(source, before);
}
