use super::*;

#[test]
fn no_output_pick_replay_metadata_preserves_history_and_redo_without_an_entry() {
    let mut doc = Document::default();
    let point = |x| Geometry::Point(Point3::try_new(x, 0., 0.).unwrap());
    doc.begin_transaction("Sources").unwrap();
    let ids = [0., 1., 2.].map(|x| doc.add_geometry(point(x)).unwrap());
    doc.commit_transaction().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    for _ in 0..2 {
        doc.begin_transaction("No output").unwrap();
        doc.release_command_selection_on_history_replay(ids)
            .unwrap();
        assert!(!doc.commit_transaction().unwrap());
    }
    assert_eq!(doc.undo_label(), Some("Sources"));
    assert_eq!(doc.selected_object_count(), 3);
    assert_eq!(doc.undo().unwrap().as_deref(), Some("Sources"));
    assert_eq!(doc.objects().len(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.selected_object_count(), 0);
    // Retaining another no-output pick does not consume an existing Redo entry.
    doc.add_geometry(point(4.)).unwrap();
    doc.undo().unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    doc.begin_transaction("No output").unwrap();
    doc.release_command_selection_on_history_replay(ids)
        .unwrap();
    assert!(!doc.commit_transaction().unwrap());
    assert_eq!(doc.redo_label(), redo.as_deref());
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 4);
}

#[test]
fn shrink_pick_release_preserves_unrelated_selection_and_rollback() {
    let mut doc = Document::default();
    let point = |x| Geometry::Point(Point3::try_new(x, 0., 0.).unwrap());
    let ids = [0., 1., 2.].map(|x| doc.add_geometry(point(x)).unwrap());
    doc.clear_history().unwrap();
    assert!(matches!(
        doc.release_command_selection_on_history_replay(ids),
        Err(DocumentError::NoActiveTransaction)
    ));
    doc.select_objects_direct([ids[0], ids[1]], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for rollback in [true, false] {
        doc.begin_transaction("Shrink picks").unwrap();
        doc.release_command_selection_on_history_replay([ids[0], ids[1]])
            .unwrap();
        doc.clear_selection();
        doc.replace_object_geometries([(ids[1], point(4.))])
            .unwrap();
        doc.select_command_results([ids[0]]).unwrap();
        if rollback {
            doc.rollback_transaction().unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert_eq!(doc.selected_object_count(), 2);
            assert!(!doc.can_undo());
        } else {
            doc.commit_transaction().unwrap();
            doc.select_objects_direct([ids[2]], SelectionMode::Add)
                .unwrap();
            for undo in [true, false, true, false] {
                if undo {
                    doc.undo().unwrap();
                } else {
                    doc.redo().unwrap();
                }
                assert!(!doc.is_selected(ids[0]));
                assert!(!doc.is_selected(ids[1]));
                assert!(doc.is_selected(ids[2]));
            }
        }
    }
}

#[test]
fn shrink_pick_release_validates_all_ids_before_recording() {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
        .unwrap();
    doc.clear_history().unwrap();
    doc.begin_transaction("Source").unwrap();
    let missing = ObjectId(Uuid::new_v4());
    let before = format!("{doc:?}");
    assert!(matches!(
        doc.release_command_selection_on_history_replay([id, missing]),
        Err(DocumentError::ObjectNotFound(_))
    ));
    assert_eq!(format!("{doc:?}"), before);
    doc.commit_transaction().unwrap();
    assert!(!doc.can_undo());
}

#[test]
fn preselected_transform_replay_accepts_explicit_reselection_after_undo() {
    let mut doc = Document::default();
    let point = |x| Geometry::Point(Point3::try_new(x, 0., 0.).unwrap());
    let source = doc.add_geometry(point(0.)).unwrap();
    let peer = doc.add_geometry(point(1.)).unwrap();
    doc.select_object(source, SelectionMode::Replace).unwrap();
    doc.clear_history().unwrap();
    assert!(matches!(
        doc.release_transform_selection_on_history_replay([source]),
        Err(DocumentError::NoActiveTransaction)
    ));
    doc.begin_transaction("Transform").unwrap();
    let before = format!("{doc:?}");
    assert!(
        doc.release_transform_selection_on_history_replay([source, ObjectId(Uuid::new_v4())])
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    doc.replace_object_geometries([(source, point(2.))])
        .unwrap();
    doc.release_transform_selection_on_history_replay([source])
        .unwrap();
    doc.commit_transaction().unwrap();
    doc.undo().unwrap();
    assert!(doc.is_selected(source));
    doc.redo().unwrap();
    assert!(!doc.is_selected(source));
    doc.undo().unwrap();
    assert!(doc.is_selected(source));
    // A repeated selection is intentional even though membership does not change.
    doc.select_object(source, SelectionMode::Replace).unwrap();
    doc.redo().unwrap();
    assert!(doc.is_selected(source));
    doc.undo().unwrap();
    doc.select_object(peer, SelectionMode::Replace).unwrap();
    doc.redo().unwrap();
    assert!(!doc.is_selected(source));
    assert!(doc.is_selected(peer));
    assert_eq!(doc.object(source).unwrap().geometry(), &point(2.));
    doc.undo().unwrap();
    doc.clear_selection();
    doc.redo().unwrap();
    assert_eq!(doc.selected_object_count(), 0);
}
