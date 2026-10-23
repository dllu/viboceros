use super::*;
use viboceros_document::SelectionMode;

#[test]
fn empty_non_mesh_and_malformed_commands_leave_history_untouched() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    assert!(matches!(
        registry.execute(&mut doc, "RebuildMeshNormals"),
        Err(CommandError::NoObjectsSelected)
    ));
    registry.execute(&mut doc, "Point 0,0,0").unwrap();
    let id = doc.objects().next().unwrap().id();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut doc, "RebuildMeshNormals"),
        Err(CommandError::UnsupportedRebuildMeshNormalsGeometry)
    ));
    assert!(
        registry
            .execute(&mut doc, "RebuildMeshNormals extra")
            .is_err()
    );
    assert_eq!(
        doc.objects().collect::<Vec<_>>(),
        objects.iter().collect::<Vec<_>>()
    );
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), vec![id]);
    assert_eq!(doc.undo_label(), undo.as_deref());
}

#[test]
fn a_late_unrepresentable_native_mesh_preserves_prior_geometry_selection_order_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let good = TriangleMesh::try_from_face_records(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap();
    let mut points = good.vertices().to_vec();
    points[0] = Point3::try_new(f64::MAX, 0., 0.).unwrap();
    let bad = TriangleMesh::try_from_face_records(points, good.faces().to_vec()).unwrap();
    let ids = [good, bad].map(|m| doc.add_geometry(Geometry::Mesh(m)).unwrap());
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let selection = doc.selected_object_ids().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    assert!(registry.execute(&mut doc, "RebuildMeshNormals").is_err());
    assert_eq!(
        doc.objects().collect::<Vec<_>>(),
        objects.iter().collect::<Vec<_>>()
    );
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selection);
    assert_eq!(doc.undo_label(), undo.as_deref());
    assert_eq!(doc.redo_label(), redo.as_deref());
}
