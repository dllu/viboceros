//! extraction mesh border command regressions.
use super::*;

#[test]
fn duplicate_mesh_edge_picks_the_nearest_logical_edge_with_fresh_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    let mesh = TriangleMesh::try_new(
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
    let expected = mesh
        .logical_edge_polylines(90.0_f64.to_radians(), document.tolerance())
        .unwrap()
        .pop()
        .unwrap();
    assert!(expected.is_closed());
    assert_eq!(expected.segment_count(), 4);
    let source = document
        .add_geometry_with_attributes(
            Geometry::Mesh(mesh),
            ObjectAttributes::on_layer(input_layer)
                .with_name("Colored mesh")
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
        registry
            .execute(&mut document, "DuplicateMeshEdge 2,-0.1,0")
            .unwrap(),
        "Duplicated 1 mesh edge curve(s) from 1 mesh(es)"
    );
    assert!(!document.is_selected(source));
    let output = document.objects().last().unwrap();
    assert!(document.is_selected(output.id()));
    assert_eq!(output.attributes().layer_id(), output_layer);
    assert_eq!(output.attributes().name(), None);
    assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
    assert!(matches!(output.geometry(), Geometry::Polyline(polyline) if polyline == &expected));
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("DupMeshEdge"));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn duplicate_mesh_edge_break_angle_separates_a_strict_crease() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 3.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 3.0).unwrap(),
        ],
        vec![[0, 1, 2], [0, 3, 1]],
        document.tolerance(),
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    registry
        .execute(&mut document, "DupMeshEdge 2,0.01,0.01 BreakAngle=90")
        .unwrap();
    let Geometry::Polyline(crease) = document.objects().last().unwrap().geometry() else {
        panic!("a picked logical mesh edge must be a polyline")
    };
    assert_eq!(crease.segment_count(), 1);
    assert_eq!(
        [crease.vertices()[0], crease.vertices()[1]],
        [
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(4.0, 0.0, 0.0).unwrap(),
        ]
    );

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut document, "DupMeshEdge 2,0.01,0.01 BreakAngle 91")
        .unwrap();
    let Geometry::Polyline(boundary) = document.objects().last().unwrap().geometry() else {
        panic!("the smoothed result must be a boundary polyline")
    };
    assert!(boundary.is_closed());
    assert_eq!(boundary.segment_count(), 4);
}

#[test]
fn duplicate_mesh_edge_all_outputs_joined_polylines_or_individual_lines() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
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
    let expected_lines = mesh
        .filtered_edge_lines(MeshEdgeFilter::Unwelded, document.tolerance())
        .unwrap();
    let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry.execute(&mut document, "DupMeshEdge All").unwrap(),
        "Duplicated 1 mesh edge curve(s) from 1 mesh(es)"
    );
    let Geometry::Polyline(joined) = document.objects().last().unwrap().geometry() else {
        panic!("the default All output must be joined polylines")
    };
    assert_eq!(joined.segment_count(), 5);
    assert!(!document.is_selected(source));

    registry.execute(&mut document, "Undo").unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "DupMeshEdge _All BreakAngle=0 Output=_Lines",)
            .unwrap(),
        "Duplicated 5 mesh edge curve(s) from 1 mesh(es)"
    );
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), expected_lines.len());
    for (output, expected) in outputs.into_iter().zip(expected_lines) {
        assert!(matches!(output.geometry(), Geometry::Line(line) if line == &expected));
    }
}

#[test]
fn duplicate_mesh_edge_rejects_empty_unsupported_and_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "DupMeshEdge All"),
        Err(CommandError::NoObjectsSelected)
    ));

    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    let line = document.objects().next().unwrap().id();
    document
        .select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupMeshEdge All"),
        Err(CommandError::UnsupportedDuplicateMeshEdgeGeometry)
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
        registry.execute(&mut document, "DupMeshEdge 0,0,0 BreakAngle=180"),
        Err(CommandError::NoDuplicateMeshEdges)
    ));
    for invalid in [
        "DupMeshEdge",
        "DupMeshEdge All Output=Curves",
        "DupMeshEdge 1,2,3 Output=Lines",
        "DupMeshEdge All BreakAngle=-1",
        "DupMeshEdge All BreakAngle=181",
        "DupMeshEdge All BreakAngle=nan",
        "DupMeshEdge All BreakAngle=20 BreakAngle=30",
        "DupMeshEdge All Output=Lines Output=Polylines",
        "DupMeshEdge All Unknown=Yes",
        "DupMeshEdge All 1,2,3",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}

#[test]
fn duplicate_mesh_hole_boundary_picks_exact_loop_and_uses_fresh_attributes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Layer New Input").unwrap();
    let input_layer = document.current_layer_id();
    let mesh = rectangular_annulus_mesh(document.tolerance());
    let expected = mesh
        .boundary_polylines(document.tolerance())
        .unwrap()
        .into_iter()
        .find(|boundary| {
            document
                .tolerance()
                .approx_eq(boundary.length().unwrap(), 16.0)
        })
        .unwrap();
    let source = document
        .add_geometry_with_attributes(
            Geometry::Mesh(mesh),
            ObjectAttributes::on_layer(input_layer)
                .with_name("Colored annulus")
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
        registry
            .execute(&mut document, "DuplicateMeshHoleBoundary 3.1,5,0")
            .unwrap(),
        "Duplicated 1 mesh hole boundary polyline(s) from 1 object(s)"
    );
    assert!(!document.is_selected(source));
    let output = document.objects().last().unwrap();
    assert!(document.is_selected(output.id()));
    assert_eq!(output.attributes().layer_id(), output_layer);
    assert_eq!(output.attributes().name(), None);
    assert_eq!(output.attributes().color_source(), ObjectColorSource::Layer);
    assert!(matches!(output.geometry(), Geometry::Polyline(boundary) if boundary == &expected));
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
    assert_eq!(document.undo_label(), Some("DupMeshHoleBoundary"));

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(
        document
            .group(source_group)
            .unwrap()
            .members()
            .collect::<Vec<_>>(),
        vec![source]
    );
}

#[test]
fn duplicate_mesh_hole_boundary_all_preserves_topology_order() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let mesh = rectangular_annulus_mesh(document.tolerance());
    let expected = mesh.boundary_polylines(document.tolerance()).unwrap();
    assert_eq!(expected.len(), 2);
    let source = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();

    assert_eq!(
        registry
            .execute(&mut document, "DupMeshHoleBoundary Boundaries=All")
            .unwrap(),
        "Duplicated 2 mesh hole boundary polyline(s) from 1 object(s)"
    );
    let outputs = document.objects().skip(1).collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    for (output, expected) in outputs.iter().zip(expected) {
        assert!(matches!(output.geometry(), Geometry::Polyline(boundary) if boundary == &expected));
    }
}

#[test]
fn duplicate_mesh_hole_boundary_rejects_invalid_input_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(matches!(
        registry.execute(&mut document, "DupMeshHoleBoundary Boundaries=All"),
        Err(CommandError::NoObjectsSelected)
    ));

    registry.execute(&mut document, "Line 0,0 1,0").unwrap();
    let line = document.objects().next().unwrap().id();
    document
        .select_objects_direct([line], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupMeshHoleBoundary Boundaries=All"),
        Err(CommandError::UnsupportedDuplicateMeshHoleBoundaryGeometry)
    ));
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.undo_label(), history.as_deref());

    let mesh = rectangular_annulus_mesh(document.tolerance());
    let mesh_id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
    document
        .select_objects_direct([mesh_id], SelectionMode::Replace)
        .unwrap();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "DupMeshHoleBoundary Boundaries=2"),
        Err(CommandError::DuplicateMeshHoleBoundaryIndexOutOfRange {
            boundary: 2,
            boundary_count: 2,
        })
    ));
    for invalid in [
        "DupMeshHoleBoundary",
        "DupMeshHoleBoundary Boundaries=",
        "DupMeshHoleBoundary Boundaries=0,0",
        "DupMeshHoleBoundary Boundaries=0 1,2,3",
        "DupMeshHoleBoundary 1,2,3 Boundaries=0",
        "DupMeshHoleBoundary BoundaryIndices=0 Boundaries=1",
        "DupMeshHoleBoundary Unknown=0",
    ] {
        assert!(
            registry.execute(&mut document, invalid).is_err(),
            "{invalid}"
        );
        assert_eq!(document.objects().len(), 2);
        assert_eq!(document.undo_label(), history.as_deref());
    }

    let mut closed_document = Document::default();
    let closed = TriangleMesh::try_new(
        vec![
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1.0, 0.0, 0.0).unwrap(),
            Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            Point3::try_new(0.0, 0.0, 1.0).unwrap(),
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        closed_document.tolerance(),
    )
    .unwrap();
    let closed_id = closed_document
        .add_geometry(Geometry::Mesh(closed))
        .unwrap();
    closed_document
        .select_objects_direct([closed_id], SelectionMode::Replace)
        .unwrap();
    let history = closed_document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut closed_document, "DupMeshHoleBoundary Boundaries=All"),
        Err(CommandError::NoDuplicateMeshHoleBoundaries)
    ));
    assert_eq!(closed_document.objects().len(), 1);
    assert_eq!(closed_document.undo_label(), history.as_deref());
}
