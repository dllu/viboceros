use super::*;
#[test]
fn invalid_mesh_patch_options_preserve_selection_geometry_and_redo() {
    let commands = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let ids = [(0., 0.), (5., 0.), (0., 4.)]
        .into_iter()
        .map(|(x, y)| {
            doc.add_geometry(Geometry::Point(Point3::try_new(x, y, 0.).unwrap()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    doc.select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    commands.execute(&mut doc, "Point 8,8,8").unwrap();
    doc.undo().unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    let parents = ids
        .iter()
        .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
        .collect::<Vec<_>>();
    for options in [
        "AngleTolerance=0",
        "AngleTolerance=NaN",
        "AngleTolerance=15 angletolerance=20",
        "MainSegmentCount=0",
        "TrimBack=Maybe",
        "Holes=invalid-id",
        "Face=2",
    ] {
        assert!(
            commands
                .execute(&mut doc, &format!("MeshPatch {options}"))
                .is_err()
        );
        assert_eq!(doc.objects().count(), 3);
        assert_eq!(doc.redo_label(), redo.as_deref());
        for (id, parent) in ids.iter().zip(&parents) {
            assert!(parent.shares_storage_with(doc.object(*id).unwrap().geometry_snapshot()));
            assert!(doc.is_selected(*id));
        }
    }
}
#[test]
fn an_oversized_point_cloud_is_declined_without_an_edit() {
    let mut doc = Document::default();
    let cloud = viboceros_geometry::PointCloud3::try_new(vec![
        Point3::try_new(0., 0., 0.).unwrap();
        MAX_MESH_PATCH_POINTS + 1
    ])
    .unwrap();
    let id = doc.add_geometry(Geometry::PointCloud(cloud)).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    let parent = doc.object(id).unwrap().geometry_snapshot().clone();
    assert!(matches!(
        CommandRegistry::with_builtins().execute(&mut doc, "MeshPatch"),
        Err(CommandError::Geometry(GeometryError::MeshPatchLimit))
    ));
    assert_eq!(doc.objects().count(), 1);
    assert!(doc.is_selected(id));
    assert!(parent.shares_storage_with(doc.object(id).unwrap().geometry_snapshot()));
    assert!(doc.undo_label().is_none());
}
