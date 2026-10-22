//! mesh edit unweld command regressions.
use super::*;

#[test]
fn unwelds_mesh_face_regions_with_identity_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Creases")
        .unwrap();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let right_angle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, 0.0, 3.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let flat_with_unused = TriangleMesh::try_new(
        vec![
            point(10.0, 0.0, 0.0),
            point(14.0, 0.0, 0.0),
            point(10.0, 3.0, 0.0),
            point(10.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let right_id = document
        .add_geometry(Geometry::Mesh(right_angle.clone()))
        .unwrap();
    let flat_id = document
        .add_geometry(Geometry::Mesh(flat_with_unused.clone()))
        .unwrap();
    let attributes = document.object(right_id).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group CreaseSet").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "Unweld Angle=90 ModifyNormals=No")
            .unwrap(),
        "Processed 1 unweld edge(s) in 2 changed mesh(es); 0 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("Unweld"));
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.object(right_id).unwrap().attributes(), &attributes);
    let Geometry::Mesh(unwelded) = document.object(right_id).unwrap().geometry() else {
        panic!("expected unwelded mesh")
    };
    assert_eq!(
        unwelded.vertices(),
        &[
            point(0.0, 3.0, 0.0),
            point(0.0, 0.0, 3.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
        ]
    );
    assert_eq!(unwelded.triangles(), &[[3, 4, 0], [5, 2, 1]]);
    let Geometry::Mesh(compacted) = document.object(flat_id).unwrap().geometry() else {
        panic!("expected compacted mesh")
    };
    assert_eq!(compacted.vertices(), &flat_with_unused.vertices()[..4]);
    assert_eq!(compacted.triangles(), flat_with_unused.triangles());
    assert_eq!(
        document
            .group_by_name("CreaseSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([right_id, flat_id])
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(right_id).unwrap().geometry(),
        &Geometry::Mesh(right_angle)
    );
    assert_eq!(
        document.object(flat_id).unwrap().geometry(),
        &Geometry::Mesh(flat_with_unused)
    );
    assert_eq!(
        registry
            .execute(&mut document, "Unweld ModifyNormals Yes")
            .unwrap(),
        "Processed 2 unweld edge(s) in 2 changed mesh(es); 0 mesh(es) unchanged"
    );
}

#[test]
fn unweld_rejects_noops_mixed_selection_and_invalid_options_atomically() {
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
        registry.execute(&mut compact_only, "Unweld"),
        Err(CommandError::NoMeshEdgesToUnweld)
    ));
    for command in [
        "Unweld -1",
        "Unweld 180.001",
        "Unweld nan",
        "Unweld Angle=20 30",
        "Unweld ModifyNormals=Maybe",
        "Unweld Unknown=20",
    ] {
        assert!(registry.execute(&mut compact_only, command).is_err());
        assert_eq!(compact_only.undo_label(), history.as_deref());
        assert_eq!(
            compact_only.object(compact_id).unwrap().geometry(),
            &Geometry::Mesh(compact.clone())
        );
    }

    let right_angle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, 0.0, 3.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(right_angle)).unwrap();
    mixed
        .add_geometry(Geometry::Point(point(9.0, 9.0, 0.0)))
        .unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "Unweld 45"),
        Err(CommandError::UnsupportedUnweldGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}

#[test]
fn unweld_edge_preserves_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Creases")
        .unwrap();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Group CreaseEdge").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "UnweldEdge Edges=0 ModifyNormals=No")
            .unwrap(),
        "Processed 1 selected mesh edge(s) in 1 changed mesh(es): 1 new seam(s); 0 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("UnweldEdge"));
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    assert!(
        document
            .group_by_name("CreaseEdge")
            .unwrap()
            .members()
            .any(|member| member == source)
    );
    let Geometry::Mesh(unwelded) = document.object(source).unwrap().geometry() else {
        panic!("expected unwelded mesh")
    };
    assert_eq!(
        unwelded.vertices(),
        &[
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
        ]
    );
    assert_eq!(unwelded.triangles(), &[[2, 5, 0], [4, 3, 1]]);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh.clone())
    );
    assert_eq!(
        registry
            .execute(&mut document, "UnweldMeshEdge 2,-0.1,0 ModifyNormals Yes",)
            .unwrap(),
        "Processed 1 selected mesh edge(s) in 1 changed mesh(es): 1 new seam(s); 0 mesh(es) unchanged"
    );

    let mut compact = Document::default();
    let with_unused = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        compact.tolerance(),
    )
    .unwrap();
    let compact_id = compact.add_geometry(Geometry::Mesh(with_unused)).unwrap();
    compact
        .select_object(compact_id, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut compact, "UnweldEdge EdgeIndices 1")
            .unwrap(),
        "Processed 1 selected mesh edge(s) in 1 changed mesh(es): 0 new seam(s); 0 mesh(es) unchanged"
    );
    let Geometry::Mesh(compacted) = compact.object(compact_id).unwrap().geometry() else {
        panic!("expected compacted mesh")
    };
    assert_eq!(compacted.vertices().len(), 4);
}

#[test]
fn unweld_edge_rejects_noops_mixed_selection_and_bad_arguments_atomically() {
    let registry = CommandRegistry::with_builtins();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let triangle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Mesh(triangle.clone()))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "UnweldEdge Edges=0"),
        Err(CommandError::NoSelectedMeshEdgesToUnweld)
    ));
    for command in [
        "UnweldEdge",
        "UnweldEdge Edges=",
        "UnweldEdge Edges=0,0",
        "UnweldEdge Edges=3",
        "UnweldEdge Edges=0 1,2,3",
        "UnweldEdge 1,2,3 Edges=0",
        "UnweldEdge ModifyNormals=Maybe Edges=0",
        "UnweldEdge Edges=0 ModifyNormals=No ModifyNormals=Yes",
        "UnweldEdge Unknown=0",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(triangle.clone())
        );
    }

    let shared = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(shared)).unwrap();
    mixed
        .add_geometry(Geometry::Point(point(9.0, 9.0, 0.0)))
        .unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "UnweldEdge Edges=0"),
        Err(CommandError::UnsupportedUnweldEdgeGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}

#[test]
fn unweld_vertex_preserves_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New CreaseVertices")
        .unwrap();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Group CreaseVertex")
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "UnweldVertex Vertices=0 ModifyNormals=No",)
            .unwrap(),
        "Processed 1 selected mesh topology vertex index(es) in 1 changed mesh(es): 1 previously shared vertex set(s) separated; 0 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("UnweldVertex"));
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    assert!(
        document
            .group_by_name("CreaseVertex")
            .unwrap()
            .members()
            .any(|member| member == source)
    );
    let Geometry::Mesh(unwelded) = document.object(source).unwrap().geometry() else {
        panic!("expected vertex-unwelded mesh")
    };
    assert_eq!(
        unwelded.vertices(),
        &[
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
        ]
    );
    assert_eq!(unwelded.triangles(), &[[4, 0, 1], [0, 3, 2]]);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        registry
            .execute(&mut document, "UnweldVertices 0.1,0.1,0 ModifyNormals Yes",)
            .unwrap(),
        "Processed 1 selected mesh topology vertex index(es) in 1 changed mesh(es): 1 previously shared vertex set(s) separated; 0 mesh(es) unchanged"
    );

    let mut compact = Document::default();
    let with_unused = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
            point(99.0, 99.0, 99.0),
        ],
        vec![[0, 1, 2]],
        compact.tolerance(),
    )
    .unwrap();
    let compact_id = compact.add_geometry(Geometry::Mesh(with_unused)).unwrap();
    compact
        .select_object(compact_id, SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut compact, "UnweldMeshVertex VertexIndices 0")
            .unwrap(),
        "Processed 1 selected mesh topology vertex index(es) in 1 changed mesh(es): 0 previously shared vertex set(s) separated; 0 mesh(es) unchanged"
    );
    let Geometry::Mesh(compacted) = compact.object(compact_id).unwrap().geometry() else {
        panic!("expected compacted mesh")
    };
    assert_eq!(compacted.vertices().len(), 3);
}

#[test]
fn unweld_vertex_rejects_noops_mixed_selection_and_bad_arguments_atomically() {
    let registry = CommandRegistry::with_builtins();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let triangle = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(0.0, 1.0, 0.0),
        ],
        vec![[0, 1, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Mesh(triangle.clone()))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "UnweldVertex Vertices=0"),
        Err(CommandError::NoSelectedMeshVerticesToUnweld)
    ));
    for command in [
        "UnweldVertex",
        "UnweldVertex Vertices=",
        "UnweldVertex Vertices=0,0",
        "UnweldVertex Vertices=3",
        "UnweldVertex Vertices=0 1,2,3",
        "UnweldVertex 1,2,3 Vertices=0",
        "UnweldVertex ModifyNormals=Maybe Vertices=0",
        "UnweldVertex Vertices=0 ModifyNormals=No ModifyNormals=Yes",
        "UnweldVertex Unknown=0",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(triangle.clone())
        );
    }

    let shared = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [1, 0, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(shared)).unwrap();
    mixed
        .add_geometry(Geometry::Point(point(9.0, 9.0, 0.0)))
        .unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "UnweldVertex Vertices=0"),
        Err(CommandError::UnsupportedUnweldVertexGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
