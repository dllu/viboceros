use super::*;

#[test]
fn temporary_grip_display_cleanup_is_deferred_scoped_and_rollback_safe() {
    let mut doc = Document::default();
    let ids = [0., 1.].map(|y| {
        doc.add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                Point3::try_new(0., y, 0.).unwrap(),
                Point3::try_new(10., y, 0.).unwrap(),
                doc.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap()
    });
    doc.enable_control_points(ids).unwrap();
    let original = doc.object(ids[0]).unwrap().geometry_snapshot().clone();
    assert!(matches!(
        doc.release_control_point_display_on_history_replay(ids),
        Err(DocumentError::NoActiveTransaction)
    ));
    doc.begin_transaction("Rollback").unwrap();
    assert!(
        doc.release_control_point_display_on_history_replay([ObjectId::new()])
            .is_err()
    );
    doc.release_control_point_display_on_history_replay([ids[0], ids[0]])
        .unwrap();
    doc.add_geometry(Geometry::Point(Point3::try_new(5., 5., 0.).unwrap()))
        .unwrap();
    doc.rollback_transaction().unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert!(
        ids.into_iter()
            .all(|id| doc.control_point_locations(id).is_some())
    );
    doc.begin_transaction("Temporary inputs").unwrap();
    doc.release_control_point_display_on_history_replay([ids[0]])
        .unwrap();
    doc.add_geometry(Geometry::Point(Point3::try_new(5., 5., 0.).unwrap()))
        .unwrap();
    doc.commit_transaction().unwrap();
    assert!(
        ids.into_iter()
            .all(|id| doc.control_point_locations(id).is_some())
    );
    doc.undo().unwrap();
    assert!(doc.control_point_locations(ids[0]).is_none());
    assert!(doc.control_point_locations(ids[1]).is_some());
    doc.enable_control_points([ids[0]]).unwrap();
    doc.redo().unwrap();
    assert!(doc.control_point_locations(ids[0]).is_none());
    assert!(doc.control_point_locations(ids[1]).is_some());
    assert!(
        doc.object(ids[0])
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
}

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

#[test]
fn grip_replay_release_is_atomic_transient_and_does_not_replace_owner_geometry() {
    let mut doc = Document::default();
    let p = |x| Point3::try_new(x, 0., 0.).unwrap();
    let owner = doc
        .add_geometry(Geometry::NurbsCurve(
            viboceros_geometry::NurbsCurve::try_clamped_uniform(2, vec![p(0.), p(1.), p(2.)])
                .unwrap(),
        ))
        .unwrap();
    doc.enable_control_points([owner]).unwrap();
    let grip = ControlPointId {
        object: owner,
        index: 1,
    };
    doc.select_control_points([grip], SelectionMode::Add)
        .unwrap();
    let original = doc.object(owner).unwrap().geometry_snapshot().clone();
    doc.clear_history().unwrap();
    assert!(matches!(
        doc.release_control_point_selection_on_history_replay([grip]),
        Err(DocumentError::NoActiveTransaction)
    ));
    doc.begin_transaction("Copies").unwrap();
    assert!(
        doc.release_control_point_selection_on_history_replay([
            grip,
            ControlPointId { index: 99, ..grip }
        ])
        .is_err()
    );
    doc.release_control_point_selection_on_history_replay([grip, grip])
        .unwrap();
    doc.add_geometry(Geometry::Point(p(5.))).unwrap();
    doc.commit_transaction().unwrap();
    assert_eq!(doc.selected_control_points().count(), 1);
    doc.undo().unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.selected_control_points().count(), 0);
    doc.redo().unwrap();
    assert_eq!(doc.selected_control_points().count(), 0);
    assert!(
        doc.object(owner)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
    doc.undo().unwrap();
    doc.select_control_points([grip], SelectionMode::Add)
        .unwrap();
    let redo = doc.redo_label().map(str::to_owned);
    doc.begin_transaction("No output").unwrap();
    doc.release_control_point_selection_on_history_replay([grip])
        .unwrap();
    assert!(!doc.commit_transaction().unwrap());
    assert_eq!(doc.redo_label(), redo.as_deref());
}
