use super::*;
use viboceros_document::SelectionMode;
fn source() -> TriangleMesh {
    TriangleMesh::try_from_face_records(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap()
    .try_with_vertex_colors(Some(vec![[10, 20, 30, 0]; 4]))
    .unwrap()
    .try_with_texture_coordinates(Some(vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]))
    .unwrap()
}

#[test]
fn command_specific_staging_is_atomic_and_does_not_commit_defaults() {
    let registry = CommandRegistry::with_builtins();
    let mut prompt = registry
        .object_selection_prompt("RebuildMesh")
        .unwrap()
        .unwrap();
    registry
        .update_object_selection_options(
            &mut prompt,
            "_PreserveTextureCoordinates _PreserveVertexColors",
        )
        .unwrap();
    assert!(prompt.options.iter().all(|o| o.value));
    assert_eq!(
        registry.rebuild_mesh_options(&[]).unwrap(),
        MeshRebuildOptions::default()
    );
    let before = prompt.clone();
    assert!(
        registry
            .update_object_selection_options(
                &mut prompt,
                "PreserveTextureCoordinates PreserveVertexColors=Maybe"
            )
            .is_err()
    );
    assert_eq!(prompt, before);
    let mut ordinary = registry
        .object_selection_prompt("MarkFoci")
        .unwrap()
        .unwrap();
    let before = ordinary.clone();
    assert!(
        registry
            .update_object_selection_options(&mut ordinary, "DrawAxis")
            .is_err()
    );
    assert_eq!(ordinary, before);
}
#[test]
fn accepted_preferences_cross_documents_but_stay_outside_undo_and_other_registries() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc.add_geometry(Geometry::Mesh(source())).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    let both = MeshRebuildOptions {
        preserve_texture_coordinates: true,
        preserve_vertex_colors: true,
    };
    registry.execute(&mut doc, &both.command()).unwrap();
    doc.undo().unwrap();
    assert_eq!(registry.rebuild_mesh_options(&[]).unwrap(), both);
    doc.redo().unwrap();
    assert_eq!(registry.rebuild_mesh_options(&[]).unwrap(), both);
    assert_eq!(
        CommandRegistry::with_builtins()
            .rebuild_mesh_options(&[])
            .unwrap(),
        MeshRebuildOptions::default()
    );
    let mut next = Document::default();
    let id = next.add_geometry(Geometry::Mesh(source())).unwrap();
    next.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    registry.execute(&mut next, "RebuildMesh").unwrap();
    let Geometry::Mesh(result) = next.object(id).unwrap().geometry() else {
        panic!();
    };
    assert!(result.vertex_colors().is_some());
    assert!(result.texture_coordinates().is_some());
    for args in [
        &["PreserveTextureCoordinates=Maybe"][..],
        &["PreserveVertexColors=Yes", "PreserveVertexColors=No"][..],
        &["Unknown=Yes"][..],
    ] {
        assert!(registry.rebuild_mesh_options(args).is_err());
        assert_eq!(registry.rebuild_mesh_options(&[]).unwrap(), both);
    }
}
#[test]
fn late_native_failure_preserves_geometry_order_selection_redo_and_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let good = source();
    let mut points = good.vertices().to_vec();
    points[0] = Point3::try_new(f64::MAX, 0., 0.).unwrap();
    let bad = TriangleMesh::try_from_face_records(points, good.faces().to_vec()).unwrap();
    let ids = [good, bad].map(|m| doc.add_geometry(Geometry::Mesh(m)).unwrap());
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let objects = doc.objects().cloned().collect::<Vec<_>>();
    let selected = doc.selected_object_ids().collect::<Vec<_>>();
    let undo = doc.undo_label().map(str::to_owned);
    let redo = doc.redo_label().map(str::to_owned);
    assert!(
        registry
            .execute(
                &mut doc,
                "RebuildMesh PreserveTextureCoordinates=Yes PreserveVertexColors=Yes"
            )
            .is_err()
    );
    assert_eq!(
        doc.objects().collect::<Vec<_>>(),
        objects.iter().collect::<Vec<_>>()
    );
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
    assert_eq!(doc.undo_label(), undo.as_deref());
    assert_eq!(doc.redo_label(), redo.as_deref());
    assert_eq!(
        registry.rebuild_mesh_options(&[]).unwrap(),
        MeshRebuildOptions::default()
    );
}
