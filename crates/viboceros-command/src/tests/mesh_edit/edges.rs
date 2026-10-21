use super::*;

#[test]
fn swaps_mesh_edge_with_rhino_order_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Retopology")
        .unwrap();
    let vertices = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 2.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        Point3::try_new(99.0, 99.0, 99.0).unwrap(),
    ];
    let mesh = TriangleMesh::try_new(
        vertices.clone(),
        vec![[0, 1, 2], [0, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let naked = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(12.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Swappable mesh")
        .with_object_color(ColorRgb::new(211, 77, 31));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let unchanged = document
        .add_geometry(Geometry::Mesh(naked.clone()))
        .unwrap();
    let group = document
        .add_group(Some("Retopology group".to_owned()), [source])
        .unwrap();
    document
        .select_objects_direct([source, unchanged], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "SwapMeshEdge Edge=1")
            .unwrap(),
        "Swapped 1 mesh edge(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert!(document.is_selected(source));
    assert!(document.is_selected(unchanged));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    let Geometry::Mesh(swapped) = document.object(source).unwrap().geometry() else {
        panic!("expected edge-swapped mesh")
    };
    assert_eq!(swapped.vertices(), vertices);
    assert_eq!(swapped.triangles(), &[[0, 1, 3], [2, 3, 1]]);
    assert_eq!(
        document.object(unchanged).unwrap().geometry(),
        &Geometry::Mesh(naked.clone())
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("SwapMeshEdge"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh.clone())
    );
    assert_eq!(
        registry
            .execute(&mut document, "SwapMeshEdge 1,1,0")
            .unwrap(),
        "Swapped 1 mesh edge(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert!(matches!(
        document.object(source).unwrap().geometry(),
        Geometry::Mesh(result) if result.triangles() == [[0, 1, 3], [2, 3, 1]]
    ));
}

#[test]
fn swap_mesh_edge_rejects_unsupported_unswappable_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "SwapMeshEdge Edge=0"),
        Err(CommandError::NoObjectsSelected)
    ));
    let triangle = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry(Geometry::Mesh(triangle.clone()))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "SwapMeshEdge Edge=0"),
        Err(CommandError::NoSwappableMeshEdges)
    ));
    assert!(matches!(
        registry.execute(&mut document, "SwapMeshEdge 1,0,0"),
        Err(CommandError::NoSwappableMeshEdges)
    ));
    assert!(matches!(
        registry.execute(&mut document, "SwapMeshEdge Edge=3"),
        Err(CommandError::SwapMeshEdgeIndexOutOfRange {
            edge: 3,
            edge_count: 3,
        })
    ));
    for invalid in [
        "SwapMeshEdge",
        "SwapMeshEdge Edge=",
        "SwapMeshEdge Edge=-1",
        "SwapMeshEdge Edge=0 Edge=1",
        "SwapMeshEdge Edge=0 1,2,3",
        "SwapMeshEdge Unknown=0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let point = document
        .add_geometry(Geometry::Point(Point3::try_new(5.0, 5.0, 5.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([source, point], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "SwapMeshEdge Edge=0"),
        Err(CommandError::UnsupportedSwapMeshEdgeGeometry)
    ));
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(triangle)
    );
}

#[test]
fn collapses_mesh_edge_to_midpoint_with_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Retopology")
        .unwrap();
    let vertices = vec![
        Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        Point3::try_new(2.0, 0.0, 0.0).unwrap(),
        Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        Point3::try_new(0.0, 0.0, 2.0).unwrap(),
        Point3::try_new(99.0, 99.0, 99.0).unwrap(),
    ];
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Collapsible mesh")
        .with_object_color(ColorRgb::new(211, 77, 31));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Retopology group".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "CollapseMeshEdge 1,0,0")
            .unwrap(),
        "Collapsed 1 mesh edge(s) in 1 mesh(es); deleted 0 empty mesh(es)"
    );
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    let Geometry::Mesh(collapsed) = document.object(source).unwrap().geometry() else {
        panic!("expected edge-collapsed mesh")
    };
    assert_eq!(
        collapsed.vertices(),
        &[
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 2.0).unwrap(),
        ]
    );
    assert_eq!(collapsed.triangles(), &[[0, 1, 2], [1, 0, 2]]);
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("CollapseMeshEdge"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn collapse_mesh_edge_deletes_empty_result_and_undo_restores_object_and_group() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    let group = document
        .add_group(Some("Disposable".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "CollapseMeshEdge Edge=0")
            .unwrap(),
        "Collapsed 1 mesh edge(s) in 1 mesh(es); deleted 1 empty mesh(es)"
    );
    assert!(document.object(source).is_none());
    assert_eq!(document.group(group).unwrap().members().len(), 0);
    assert_eq!(document.undo_label(), Some("CollapseMeshEdge"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn collapse_mesh_edge_rejects_invalid_unsupported_and_degenerate_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "CollapseMeshEdge Edge=0"),
        Err(CommandError::NoObjectsSelected)
    ));
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, -2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(2.0, -1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 3, 4]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "CollapseMeshEdge 1,0,0"),
        Err(CommandError::Geometry(GeometryError::DegenerateTriangle {
            triangle: 0
        }))
    ));
    assert!(matches!(
        registry.execute(&mut document, "CollapseMeshEdge Edge=6"),
        Err(CommandError::CollapseMeshEdgeIndexOutOfRange {
            edge: 6,
            edge_count: 6,
        })
    ));
    for invalid in [
        "CollapseMeshEdge",
        "CollapseMeshEdge Edge=",
        "CollapseMeshEdge Edge=-1",
        "CollapseMeshEdge Edge=0 Edge=1",
        "CollapseMeshEdge Edge=0 1,2,3",
        "CollapseMeshEdge Unknown=0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let point = document
        .add_geometry(Geometry::Point(Point3::try_new(5.0, 5.0, 5.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([source, point], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "CollapseMeshEdge Edge=0"),
        Err(CommandError::UnsupportedCollapseMeshEdgeGeometry)
    ));
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
}

#[test]
fn splits_mesh_edge_from_two_points_with_identity_groups_selection_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Retopology")
        .unwrap();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        document.tolerance(),
    )
    .unwrap();
    let attributes = ObjectAttributes::on_layer(document.current_layer_id())
        .with_name("Split target")
        .with_object_color(ColorRgb::new(211, 77, 31));
    let source = document
        .add_geometry_with_attributes(Geometry::Mesh(mesh.clone()), attributes.clone())
        .unwrap();
    let group = document
        .add_group(Some("Retopology group".to_owned()), [source])
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "SplitMeshEdge 2,0,0 1,0,0")
            .unwrap(),
        "Split 1 mesh edge(s) in 1 mesh(es)"
    );
    assert!(document.is_selected(source));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    let Geometry::Mesh(split) = document.object(source).unwrap().geometry() else {
        panic!("expected edge-split mesh")
    };
    assert_eq!(
        split.vertices(),
        &[
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 4.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
        ]
    );
    assert_eq!(
        split.triangles(),
        &[
            [1, 2, 3],
            [2, 0, 3],
            [2, 4, 0],
            [2, 1, 4],
            [3, 0, 4],
            [3, 4, 1],
        ]
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("SplitMeshEdge"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
    assert_eq!(
        document.group(group).unwrap().members().collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn split_mesh_edge_supports_explicit_endpoint_parameters() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let second = document
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new(
                vec![
                    Point3::try_new(10.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(14.0, 0.0, 0.0).unwrap(),
                    Point3::try_new(10.0, 4.0, 0.0).unwrap(),
                ],
                vec![[0, 1, 2]],
                document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    document
        .select_objects_direct([source, second], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "SplitMeshEdge Edge=0 Parameter=0")
            .unwrap(),
        "Split 2 mesh edge(s) in 2 mesh(es)"
    );
    for id in [source, second] {
        let Geometry::Mesh(split) = document.object(id).unwrap().geometry() else {
            panic!("expected endpoint-split mesh")
        };
        assert_eq!(split.triangles(), &[[0, 1, 2], [2, 3, 1]]);
        assert_eq!(split.vertices()[3], split.vertices()[0]);
    }
}

#[test]
fn split_mesh_edge_rejects_invalid_unsupported_and_degenerate_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "SplitMeshEdge Edge=0 Parameter=0.5"),
        Err(CommandError::NoObjectsSelected)
    ));
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 4.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "SplitMeshEdge Edge=3 Parameter=0.5"),
        Err(CommandError::SplitMeshEdgeIndexOutOfRange {
            edge: 3,
            edge_count: 3,
        })
    ));
    assert!(matches!(
        registry.execute(&mut document, "SplitMeshEdge Edge=0 Parameter=-0.1"),
        Err(CommandError::InvalidMeshEdgeSplitParameter(parameter))
            if parameter == -0.1
    ));
    assert!(matches!(
        registry.execute(&mut document, "SplitMeshEdge 2,0,0 0.000000000001,0,0"),
        Err(CommandError::Geometry(GeometryError::DegenerateTriangle {
            triangle: 0
        }))
    ));
    for invalid in [
        "SplitMeshEdge",
        "SplitMeshEdge 1,0,0",
        "SplitMeshEdge Edge=0",
        "SplitMeshEdge Parameter=0.5",
        "SplitMeshEdge Edge=-1 Parameter=0.5",
        "SplitMeshEdge Edge=0 Parameter=nan",
        "SplitMeshEdge Edge=0 Parameter=0.5 1,2,3",
        "SplitMeshEdge Edge=0 Edge=1 Parameter=0.5",
        "SplitMeshEdge Unknown=0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
    }
    assert_eq!(
        document.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(document.undo_label(), history.as_deref());

    let point = document
        .add_geometry(Geometry::Point(Point3::try_new(5.0, 5.0, 5.0).unwrap()))
        .unwrap();
    document
        .select_objects_direct([source, point], SelectionMode::Replace)
        .unwrap();
    assert!(matches!(
        registry.execute(&mut document, "SplitMeshEdge Edge=0 Parameter=0.5"),
        Err(CommandError::UnsupportedSplitMeshEdgeGeometry)
    ));
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(mesh)
    );
}
