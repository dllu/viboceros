//! mesh edit weld command regressions.
use super::*;

#[test]
fn weld_edge_preserves_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New WeldedEdges")
        .unwrap();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut document, "Group WeldedEdge").unwrap();

    assert_eq!(
        registry.execute(&mut document, "WeldEdge Edges=0").unwrap(),
        "Processed 1 selected mesh edge(s) in 1 changed mesh(es): welded 1 divided edge(s) and removed 2 raw vertex occurrence(s); 0 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("WeldEdge"));
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    assert!(
        document
            .group_by_name("WeldedEdge")
            .unwrap()
            .members()
            .any(|member| member == source)
    );
    let Geometry::Mesh(welded) = document.object(source).unwrap().geometry() else {
        panic!("expected edge-welded mesh")
    };
    assert_eq!(
        welded.vertices(),
        &[
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, -3.0, 0.0),
        ]
    );
    assert_eq!(welded.triangles(), &[[0, 1, 2], [1, 0, 3]]);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        registry
            .execute(&mut document, "WeldMeshEdge 2,-0.1,0")
            .unwrap(),
        "Processed 1 selected mesh edge(s) in 1 changed mesh(es): welded 1 divided edge(s) and removed 2 raw vertex occurrence(s); 0 mesh(es) unchanged"
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
            .execute(&mut compact, "WeldEdge EdgeIndices 0")
            .unwrap(),
        "Processed 1 selected mesh edge(s) in 1 changed mesh(es): welded 0 divided edge(s) and removed 1 raw vertex occurrence(s); 0 mesh(es) unchanged"
    );
    let Geometry::Mesh(compacted) = compact.object(compact_id).unwrap().geometry() else {
        panic!("expected compacted mesh")
    };
    assert_eq!(compacted.vertices().len(), 3);
}

#[test]
fn weld_edge_rejects_noops_mixed_selection_and_bad_arguments_atomically() {
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
        registry.execute(&mut document, "WeldEdge Edges=0"),
        Err(CommandError::NoSelectedMeshEdgesToWeld)
    ));
    for command in [
        "WeldEdge",
        "WeldEdge Edges=",
        "WeldEdge Edges=0,0",
        "WeldEdge Edges=3",
        "WeldEdge Edges=0 1,2,3",
        "WeldEdge 1,2,3 Edges=0",
        "WeldEdge Unknown=0",
        "WeldEdge Edges=0 Edges=1",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(triangle.clone())
        );
    }

    let divided = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(divided)).unwrap();
    mixed
        .add_geometry(Geometry::Point(point(9.0, 9.0, 0.0)))
        .unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "WeldEdge Edges=0"),
        Err(CommandError::UnsupportedWeldEdgeGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}

#[test]
fn weld_vertices_preserves_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New WeldedVertices")
        .unwrap();
    let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "Group WeldedVertex")
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "WeldVertices Vertices=0")
            .unwrap(),
        "Processed 1 selected mesh topology vertex index(es) in 1 changed mesh(es): welded 1 incident seam edge(s) and removed 2 raw vertex occurrence(s); 0 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("WeldVertices"));
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    assert!(
        document
            .group_by_name("WeldedVertex")
            .unwrap()
            .members()
            .any(|member| member == source)
    );
    let Geometry::Mesh(welded) = document.object(source).unwrap().geometry() else {
        panic!("expected vertex-welded mesh")
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

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        registry
            .execute(&mut document, "WeldVertex 0.1,0.1,0")
            .unwrap(),
        "Processed 1 selected mesh topology vertex index(es) in 1 changed mesh(es): welded 1 incident seam edge(s) and removed 2 raw vertex occurrence(s); 0 mesh(es) unchanged"
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
            .execute(&mut compact, "WeldMeshVertex VertexIndices 2")
            .unwrap(),
        "Processed 1 selected mesh topology vertex index(es) in 1 changed mesh(es): welded 0 incident seam edge(s) and removed 1 raw vertex occurrence(s); 0 mesh(es) unchanged"
    );
    let Geometry::Mesh(compacted) = compact.object(compact_id).unwrap().geometry() else {
        panic!("expected compacted mesh")
    };
    assert_eq!(compacted.vertices().len(), 3);
}

#[test]
fn weld_vertices_rejects_noops_mixed_selection_and_bad_arguments_atomically() {
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
        registry.execute(&mut document, "WeldVertices Vertices=0"),
        Err(CommandError::NoSelectedMeshVerticesToWeld)
    ));
    for command in [
        "WeldVertices",
        "WeldVertices Vertices=",
        "WeldVertices Vertices=0,0",
        "WeldVertices Vertices=3",
        "WeldVertices Vertices=0 1,2,3",
        "WeldVertices 1,2,3 Vertices=0",
        "WeldVertices Unknown=0",
        "WeldVertices Vertices=0 Vertices=1",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(document.undo_label(), history.as_deref());
        assert_eq!(
            document.object(source).unwrap().geometry(),
            &Geometry::Mesh(triangle.clone())
        );
    }

    let divided = TriangleMesh::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, -3.0, 0.0),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(divided)).unwrap();
    mixed
        .add_geometry(Geometry::Point(point(9.0, 9.0, 0.0)))
        .unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "WeldVertices Vertices=0"),
        Err(CommandError::UnsupportedWeldVerticesGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}
