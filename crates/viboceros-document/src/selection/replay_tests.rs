use super::*;

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
