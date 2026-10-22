//! mesh edit cleanup command regressions.
use super::*;

#[test]
fn combines_identical_mesh_vertices_with_identity_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Cleanup")
        .unwrap();
    let duplicated = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, -2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(99.0, 99.0, 99.0).unwrap(),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let unique = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry(Geometry::Mesh(duplicated.clone()))
        .unwrap();
    let unique_id = document
        .add_geometry(Geometry::Mesh(unique.clone()))
        .unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group CleanupSet").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "CombineIdenticalMeshVertices")
            .unwrap(),
        "Combined 3 identical vertex occurrence(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("CombineIdenticalMeshVertices"));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    let Geometry::Mesh(combined) = document.object(source).unwrap().geometry() else {
        panic!("expected combined mesh")
    };
    assert_eq!(
        combined.vertices(),
        &[
            Point3::try_new(99.0, 99.0, 99.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, -2.0, 0.0).unwrap(),
        ]
    );
    assert_eq!(combined.triangles(), &[[3, 1, 2], [1, 3, 4]]);
    assert_eq!(
        document.object(unique_id).unwrap().geometry(),
        &Geometry::Mesh(unique.clone())
    );
    assert_eq!(
        document
            .group_by_name("CleanupSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, unique_id])
    );
    assert_eq!(document.selected_object_count(), 2);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(duplicated)
    );

    let mut unique_only = Document::default();
    let unique_only_id = unique_only.add_geometry(Geometry::Mesh(unique)).unwrap();
    unique_only
        .select_object(unique_only_id, SelectionMode::Replace)
        .unwrap();
    let history = unique_only.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut unique_only, "CombineIdenticalMeshVertices"),
        Err(CommandError::NoIdenticalMeshVertices)
    ));
    assert_eq!(unique_only.undo_label(), history.as_deref());

    registry.execute(&mut unique_only, "Point 9,9").unwrap();
    registry.execute(&mut unique_only, "SelAll").unwrap();
    let before = unique_only.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut unique_only, "CombineIdenticalMeshVertices"),
        Err(CommandError::UnsupportedCombineIdenticalMeshVerticesGeometry)
    ));
    assert_eq!(
        unique_only.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}

#[test]
fn culls_unused_mesh_vertices_with_identity_groups_and_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Cleanup")
        .unwrap();
    let with_unused = TriangleMesh::try_new(
        vec![
            Point3::try_new(99.0, 99.0, 99.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(98.0, 98.0, 98.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(97.0, 97.0, 97.0).unwrap(),
        ],
        vec![[1, 3, 4], [3, 5, 4]],
        document.tolerance(),
    )
    .unwrap();
    let compact = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(11.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 1.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2]],
        document.tolerance(),
    )
    .unwrap();
    let source = document
        .add_geometry(Geometry::Mesh(with_unused.clone()))
        .unwrap();
    let compact_id = document
        .add_geometry(Geometry::Mesh(compact.clone()))
        .unwrap();
    let attributes = document.object(source).unwrap().attributes().clone();
    registry.execute(&mut document, "SelMesh").unwrap();
    registry.execute(&mut document, "Group CleanupSet").unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "CullUnusedMeshVertices")
            .unwrap(),
        "Culled 3 unused vertex occurrence(s) in 1 mesh(es); 1 mesh(es) unchanged"
    );
    assert_eq!(document.undo_label(), Some("CullUnusedMeshVertices"));
    assert_eq!(document.object(source).unwrap().attributes(), &attributes);
    let Geometry::Mesh(culled) = document.object(source).unwrap().geometry() else {
        panic!("expected culled mesh")
    };
    assert_eq!(
        culled.vertices(),
        &[
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(2.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 2.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        ]
    );
    assert_eq!(culled.triangles(), &[[0, 1, 2], [1, 3, 2]]);
    assert_eq!(
        document.object(compact_id).unwrap().geometry(),
        &Geometry::Mesh(compact.clone())
    );
    assert_eq!(
        document
            .group_by_name("CleanupSet")
            .unwrap()
            .members()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([source, compact_id])
    );
    assert_eq!(document.selected_object_count(), 2);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        document.object(source).unwrap().geometry(),
        &Geometry::Mesh(with_unused.clone())
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.undo_label(), Some("CullUnusedMeshVertices"));

    let mut compact_only = Document::default();
    let compact_only_id = compact_only
        .add_geometry(Geometry::Mesh(compact.clone()))
        .unwrap();
    compact_only
        .select_object(compact_only_id, SelectionMode::Replace)
        .unwrap();
    let history = compact_only.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut compact_only, "CullUnusedMeshVertices"),
        Err(CommandError::NoUnusedMeshVertices)
    ));
    assert_eq!(compact_only.undo_label(), history.as_deref());

    let mut mixed = Document::default();
    mixed.add_geometry(Geometry::Mesh(with_unused)).unwrap();
    registry.execute(&mut mixed, "Point 9,9").unwrap();
    registry.execute(&mut mixed, "SelAll").unwrap();
    let before = mixed.objects().cloned().collect::<Vec<_>>();
    assert!(matches!(
        registry.execute(&mut mixed, "CullUnusedMeshVertices"),
        Err(CommandError::UnsupportedCullUnusedMeshVerticesGeometry)
    ));
    assert_eq!(
        mixed.objects().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
}

#[test]
fn extract_mesh_edges_defaults_to_unwelded_lines_with_fresh_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [3, 4, 5]],
        document.tolerance(),
    )
    .unwrap();
    let expected = mesh
        .filtered_edge_lines(MeshEdgeFilter::Unwelded, document.tolerance())
        .unwrap();
    assert_eq!(expected.len(), 5);
    let source = document
        .add_geometry_with_attributes(
            Geometry::Mesh(mesh),
            ObjectAttributes::on_layer(input_layer)
                .with_name("Unwelded source")
                .with_object_color(ColorRgb::new(17, 83, 149)),
        )
        .unwrap();
    let source_group = document
        .add_group(Some("Source group".to_owned()), [source])
        .unwrap();
    registry.execute(&mut document, "Layer New Output").unwrap();
    let output_layer = document.current_layer_id();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "ExtractMeshEdges").unwrap(),
        "Extracted 5 mesh edge curve(s) from 1 mesh(es)"
    );
    assert!(document.is_selected(source));
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 5);
    for (output, expected) in outputs.iter().zip(expected) {
        assert!(document.is_selected(output.id()));
        assert_eq!(output.attributes().layer_id(), output_layer);
        assert_eq!(output.attributes().name(), None);
        assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
        assert!(matches!(output.geometry(), Geometry::Line(line) if line == &expected));
    }
    assert_eq!(document.selected_object_count(), 6);
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("ExtractMeshEdges"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert!(document.is_selected(source));
}

#[test]
fn extract_mesh_edges_joins_naked_loops_and_filters_face_angles() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let flat = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let flat_id = document.add_geometry(Geometry::Mesh(flat)).unwrap();
    document
        .select_objects_direct([flat_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(
                &mut document,
                "ExtractMeshEdges ExtractBy=Naked JoinResults=Yes"
            )
            .unwrap(),
        "Extracted 1 mesh edge curve(s) from 1 mesh(es)"
    );
    let Geometry::Polyline(boundary) = document.objects().last().unwrap().geometry() else {
        panic!("joined naked edges must form a polyline")
    };
    assert!(boundary.is_closed());
    assert_eq!(boundary.segment_count(), 4);

    registry.execute(&mut document, "Undo").unwrap();
    let folded = TriangleMesh::try_new(
        vec![
            Point3::try_new(10.0, 0.0, 0.0).unwrap(),
            Point3::try_new(14.0, 0.0, 0.0).unwrap(),
            Point3::try_new(10.0, 3.0, 0.0).unwrap(),
            Point3::try_new(10.0, 0.0, 3.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 3, 1]],
        document.tolerance(),
    )
    .unwrap();
    let expected = folded
        .filtered_edge_lines(
            MeshEdgeFilter::FaceAngle {
                greater_than_radians: 89.0_f64.to_radians(),
                less_than_radians: 91.0_f64.to_radians(),
            },
            document.tolerance(),
        )
        .unwrap();
    let folded_id = document.add_geometry(Geometry::Mesh(folded)).unwrap();
    document
        .select_objects_direct([folded_id], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
            registry
                .execute(
                    &mut document,
                    "ExtractMeshEdges ExtractBy=BreakAngle GreaterThanAngle=89 LessThanAngle=91 JoinResults=No"
                )
                .unwrap(),
            "Extracted 1 mesh edge curve(s) from 1 mesh(es)"
        );
    assert_eq!(expected.len(), 1);
    assert!(
        matches!(document.objects().last().unwrap().geometry(), Geometry::Line(line) if line == &expected[0])
    );
    assert!(document.is_selected(folded_id));
    assert!(!document.is_selected(flat_id));
}

#[test]
fn extract_mesh_edges_rejects_empty_unsupported_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "ExtractMeshEdges"),
        Err(CommandError::NoObjectsSelected)
    ));
    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    let line = document.objects().next().unwrap().id();
    document
        .select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "ExtractMeshEdges"),
        Err(CommandError::UnsupportedExtractMeshEdgesGeometry)
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());

    let closed = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        document.tolerance(),
    )
    .unwrap();
    let closed_id = document.add_geometry(Geometry::Mesh(closed)).unwrap();
    document
        .select_objects_direct([closed_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "ExtractMeshEdges"),
        Err(CommandError::NoExtractableMeshEdges)
    ));
    for invalid in [
        "ExtractMeshEdges ExtractBy=Creases",
        "ExtractMeshEdges ExtractBy=Naked GreaterThanAngle=10",
        "ExtractMeshEdges ExtractBy=BreakAngle GreaterThanAngle=-1",
        "ExtractMeshEdges ExtractBy=BreakAngle GreaterThanAngle=30 LessThanAngle=30",
        "ExtractMeshEdges ExtractBy=BreakAngle LessThanAngle=181",
        "ExtractMeshEdges ExtractBy=BreakAngle GreaterThanAngle=nan",
        "ExtractMeshEdges JoinResults=Maybe",
        "ExtractMeshEdges ExtractBy=Naked ExtractBy=Unwelded",
        "ExtractMeshEdges extra",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}
