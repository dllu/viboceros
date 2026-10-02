use super::*;
fn cube(doc: &mut Document) -> ObjectId {
    doc.add_geometry(Geometry::Brep(
        Brep::try_box(
            CommandContext::default().construction_plane,
            [[0., 2.], [0., 3.], [0., 5.]],
            doc.tolerance(),
        )
        .unwrap(),
    ))
    .unwrap()
}
#[test]
fn separated_components_inherit_metadata_groups_and_user_text_and_roundtrip_atomically() {
    let mut doc = Document::default();
    let id = cube(&mut doc);
    let peer = doc
        .add_geometry(Geometry::Point(Point3::try_new(20., 30., 40.).unwrap()))
        .unwrap();
    let group = doc.add_group(Some("Source".into()), [id]).unwrap();
    doc.set_object_geometry_user_text([id], "material", Some("brass"))
        .unwrap();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let selection =
        UnjoinEdgeSelection::prepare(&doc, [(id, 0), (id, 1), (id, 2), (id, 3)]).unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    let result = selection.commit(&mut doc).unwrap();
    assert_eq!(result.updated, [id]);
    assert_eq!(result.added.len(), 1);
    let output = doc.object(result.added[0]).unwrap();
    assert_eq!(output.attributes(), doc.object(id).unwrap().attributes());
    assert_eq!(output.group_ids(), [group]);
    assert_eq!(output.geometry_user_text()["material"], "brass");
    assert_eq!(
        doc.objects().map(|o| o.id()).collect::<Vec<_>>(),
        [peer, id, result.added[0]]
    );
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn stale_or_invalid_batches_and_noops_preserve_geometry_and_redo() {
    let mut doc = Document::default();
    let id = cube(&mut doc);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 4,5,6").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for tail in ["", "-1", "0,12", "0,", "0 extra", "x"] {
        assert!(
            registry
                .execute(&mut doc, &format!("UnjoinEdge {id} {tail}"))
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
    assert!(
        !UnjoinEdgeSelection::prepare(&doc, [])
            .unwrap()
            .changes_geometry()
    );
    UnjoinEdgeSelection::prepare(&doc, [])
        .unwrap()
        .commit(&mut doc)
        .unwrap();
    assert!(doc.can_redo());
    let selection = UnjoinEdgeSelection::prepare(&doc, [(id, 0)]).unwrap();
    doc.set_objects_locked([id], true).unwrap();
    let snapshot = format!("{doc:?}");
    assert!(matches!(
        selection.commit(&mut doc),
        Err(CommandError::UnjoinEdgeStale)
    ));
    assert_eq!(format!("{doc:?}"), snapshot);
}
#[test]
fn typed_multi_source_command_is_one_undo_and_deduplicates_component_indices() {
    let mut doc = Document::default();
    let first = cube(&mut doc);
    let second = cube(&mut doc);
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    CommandRegistry::with_builtins()
        .execute(
            &mut doc,
            &format!("UnjoinEdge {second} 0,0,1,2,3 {first} 0,1,2,3"),
        )
        .unwrap();
    assert_eq!(doc.objects().len(), 4);
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
}
