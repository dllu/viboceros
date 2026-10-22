//! mesh edit normals command regressions.
use super::*;

#[test]
fn unifies_and_flips_mesh_normals_without_changing_identity_or_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Printable")
        .unwrap();
    let vertices = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 1.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 1.0).unwrap(),
    ];
    let oriented_faces = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let mut inconsistent_faces = oriented_faces.clone();
    inconsistent_faces[1].swap(1, 2);
    let inconsistent = TriangleMesh::try_new(
        vertices.clone(),
        inconsistent_faces.clone(),
        document.tolerance(),
    )
    .unwrap();
    let oriented =
        TriangleMesh::try_new(vertices, oriented_faces.clone(), document.tolerance()).unwrap();
    let first = document.add_geometry(Geometry::Mesh(inconsistent)).unwrap();
    let second = document
        .add_geometry(Geometry::Mesh(oriented.clone()))
        .unwrap();
    let attributes = document.object(first).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group Shells").unwrap();

    assert_eq!(
        registry.execute(&mut document, "UnifyMeshNormals").unwrap(),
        "Unified 2 mesh(es): flipped 1 face(s) in 1 mesh(es); 1 already consistent"
    );
    assert_eq!(document.undo_label(), Some("UnifyMeshNormals"));
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.object(first).unwrap().attributes(), &attributes);
    assert_eq!(
        document
            .group_by_name("Shells")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        [first, second].into_iter().collect()
    );
    for id in [first, second] {
        let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
            panic!("expected mesh")
        };
        assert!(mesh.topology().is_oriented());
        assert_eq!(mesh.triangles(), oriented_faces);
    }

    registry.execute(&mut document, "Undo").unwrap();
    let Geometry::Mesh(restored) = document.object(first).unwrap().geometry() else {
        panic!("expected restored mesh")
    };
    assert_eq!(restored.triangles(), inconsistent_faces);
    assert!(!restored.topology().is_oriented());
    registry.execute(&mut document, "Redo").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Flip").unwrap(),
        "Flipped 2 object(s)"
    );
    for id in [first, second] {
        let Geometry::Mesh(mesh) = document.object(id).unwrap().geometry() else {
            panic!("expected flipped mesh")
        };
        assert_eq!(mesh, &oriented.reversed());
    }
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.undo_label(), Some("UnifyMeshNormals"));

    registry.execute(&mut document, "Point 9,9").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut document, "UnifyMeshNormals"),
        Err(CommandError::UnsupportedUnifyMeshNormalsGeometry)
    ));
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}

#[test]
fn welds_mesh_edges_by_angle_with_rhino_order_identity_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Cleanup")
        .unwrap();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let flat = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let right_angle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 3.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let flat_id = document.add_geometry(Geometry::Mesh(flat.clone())).unwrap();
    let right_id = document
        .add_geometry(Geometry::Mesh(right_angle.clone()))
        .unwrap();
    let attributes = document.object(flat_id).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group WeldSet").unwrap();

    assert_eq!(
        registry.execute(&mut document, "Weld Angle=90").unwrap(),
        "Welded 3 vertex occurrence(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("Weld"));
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.object(flat_id).unwrap().attributes(), &attributes);
    let Geometry::Mesh(welded) = document.object(flat_id).unwrap().geometry() else {
        panic!("expected welded mesh")
    };
    assert_eq!(
        welded.vertices(),
        &[
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ]
    );
    assert_eq!(welded.triangles(), &[[2, 1, 0], [1, 2, 3]]);
    assert_eq!(
        document.object(right_id).unwrap().geometry(),
        &Geometry::Mesh(right_angle.clone())
    );
    assert_eq!(
        document
            .group_by_name("WeldSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([flat_id, right_id])
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(flat_id).unwrap().geometry(),
        &Geometry::Mesh(flat.clone())
    );
    assert_eq!(
        registry.execute(&mut document, "Weld").unwrap(),
        "Welded 5 vertex occurrence(s) in 2 mesh(es); 0 mesh(es) unchanged"
    );
    let Geometry::Mesh(welded_right) = document.object(right_id).unwrap().geometry() else {
        panic!("expected welded mesh")
    };
    assert_eq!(welded_right.triangles(), &[[2, 1, 0], [1, 2, 3]]);
}

#[test]
fn weld_rejects_noops_mixed_selection_and_invalid_angles_atomically() {
    let registry = CommandRegistry::with_builtins();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let compact = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut compact_only = Document::default();
    let compact_id = compact_only
        .add_geometry(Geometry::Mesh(compact.clone()))
        .unwrap();
    compact_only
        .select_object(compact_id, SelectionMode::Replace)
        .unwrap();
    let history = compact_only.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut compact_only, "Weld AngleTolerance 45"),
        Err(CommandError::NoMeshVerticesToWeld)
    ));
    assert_eq!(compact_only.undo_label(), history.as_deref());
    assert_eq!(
        compact_only.object(compact_id).unwrap().geometry(),
        &Geometry::Mesh(compact)
    );
    for command in [
        "Weld -1",
        "Weld 180.001",
        "Weld nan",
        "Weld Angle=20 Angle=30",
        "Weld Unknown=20",
    ] {
        assert!(registry.execute(&mut compact_only, command).is_err());
        assert_eq!(compact_only.undo_label(), history.as_deref());
    }

    let duplicated = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 2.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -2.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(duplicated)).unwrap();
    mixed
        .add_geometry(Geometry::Point(point(9.0, 9.0, 0.0)))
        .unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "Weld 180"),
        Err(CommandError::UnsupportedWeldGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
