use crate::*;

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}

fn step(document: &mut Document, group: &mut HistoryGroup, x: f64) -> ObjectId {
    document.begin_group_transaction(group).unwrap();
    let id = document.add_geometry(point(x)).unwrap();
    assert!(document.commit_group_transaction(group).unwrap());
    id
}

#[test]
fn accepted_steps_have_one_external_undo_and_redo_and_local_undo_discards_only_the_suffix() {
    let mut document = Document::default();
    let peer = document.add_geometry(point(-1.)).unwrap();
    let initial = document.objects().cloned().collect::<Vec<_>>();
    let mut group = document.begin_history_group("Incremental").unwrap();
    let first = step(&mut document, &mut group, 1.);
    document
        .select_object(peer, SelectionMode::Replace)
        .unwrap();
    document.begin_group_transaction(&group).unwrap();
    let second = document.add_geometry(point(2.)).unwrap();
    document.add_group(None, [first, second]).unwrap();
    document
        .move_objects_to_end_in_order([peer, first])
        .unwrap();
    document.commit_group_transaction(&mut group).unwrap();
    assert!(document.undo_history_group(&mut group).unwrap());
    assert!(document.is_selected(peer));
    assert!(document.object(second).is_none());
    assert_eq!(document.groups().len(), 0);
    assert_eq!(
        document
            .objects()
            .map(|object| object.id())
            .collect::<Vec<_>>(),
        [peer, first]
    );
    let third = step(&mut document, &mut group, 3.);
    let after = document.objects().cloned().collect::<Vec<_>>();
    drop(group);
    assert_eq!(document.undo().unwrap().as_deref(), Some("Incremental"));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), initial);
    assert_eq!(document.undo_label(), Some("Add object"));
    document.redo().unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    assert!(document.object(third).is_some());
    assert!(!document.can_redo());
}

#[test]
fn empty_and_failed_steps_preserve_redo_and_token_and_undo_all_preserves_older_history() {
    let mut document = Document::default();
    document.add_geometry(point(-1.)).unwrap();
    document.add_geometry(point(-2.)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    let mut group = document.begin_history_group("Incremental").unwrap();
    document.begin_group_transaction(&group).unwrap();
    assert!(!document.commit_group_transaction(&mut group).unwrap());
    document.begin_group_transaction(&group).unwrap();
    document.add_geometry(point(1.)).unwrap();
    document.rollback_transaction().unwrap();
    assert_eq!(format!("{document:?}"), before);
    assert!(document.history_group_is_current(&group));
    assert!(!document.undo_history_group(&mut group).unwrap());
    step(&mut document, &mut group, 2.);
    assert!(!document.can_redo());
    document.undo_history_group(&mut group).unwrap();
    assert!(!group.can_undo());
    assert!(!document.undo_history_group(&mut group).unwrap());
    assert_eq!(document.undo_label(), Some("Add object"));
    step(&mut document, &mut group, 3.);
    assert_eq!(document.undo_label(), Some("Incremental"));
}

#[test]
fn external_changes_and_wrong_owners_cannot_append_or_undo_someone_elses_history() {
    for change in 0..4 {
        let mut document = Document::default();
        let mut group = document.begin_history_group("Incremental").unwrap();
        step(&mut document, &mut group, 1.);
        match change {
            0 => {
                document.add_geometry(point(2.)).unwrap();
            }
            1 => {
                document.undo().unwrap();
            }
            2 => {
                document.undo().unwrap();
                document.redo().unwrap();
            }
            _ => {
                document.clear_history().unwrap();
            }
        }
        let before = format!("{document:?}");
        assert!(!document.history_group_is_current(&group));
        assert_eq!(
            document.begin_group_transaction(&group),
            Err(DocumentError::HistoryGroupStale)
        );
        assert_eq!(
            document.undo_history_group(&mut group),
            Err(DocumentError::HistoryGroupStale)
        );
        assert_eq!(format!("{document:?}"), before);
    }
    let mut document = Document::default();
    let mut owner = document.begin_history_group("Owner").unwrap();
    let mut other = document.begin_history_group("Other").unwrap();
    document.begin_group_transaction(&owner).unwrap();
    document.add_geometry(point(1.)).unwrap();
    assert_eq!(
        document.commit_group_transaction(&mut other),
        Err(DocumentError::HistoryGroupStale)
    );
    assert_eq!(
        document.commit_transaction(),
        Err(DocumentError::HistoryGroupStale)
    );
    document.commit_group_transaction(&mut owner).unwrap();
    assert_eq!(document.undo_label(), Some("Owner"));
}

#[test]
fn many_steps_count_as_one_history_entry_and_checkpoints_store_only_incremental_ids() {
    let mut document = Document::default();
    for x in 0..100 {
        document.add_geometry(point(x as f64)).unwrap();
    }
    let mut group = document.begin_history_group("Incremental").unwrap();
    for x in 0..150 {
        step(&mut document, &mut group, 1000. + x as f64);
    }
    assert_eq!(document.history.undo.len(), 100);
    assert_eq!(
        group
            .checkpoints
            .iter()
            .map(|c| c.added_ids.len())
            .sum::<usize>(),
        150
    );
    assert_eq!(
        group
            .checkpoints
            .iter()
            .map(|c| c.affected_ids.len())
            .sum::<usize>(),
        150
    );
    for _ in 0..149 {
        document.undo_history_group(&mut group).unwrap();
    }
    document.undo().unwrap();
    assert_eq!(document.objects().len(), 100);
    document.redo().unwrap();
    assert_eq!(document.objects().len(), 101);
}

#[test]
fn failing_local_undo_restores_applied_edits_and_keeps_its_checkpoint() {
    let mut document = Document::default();
    let mut group = document.begin_history_group("Incremental").unwrap();
    let first = step(&mut document, &mut group, 1.);
    document.begin_group_transaction(&group).unwrap();
    document
        .replace_object_geometries([(first, point(3.))])
        .unwrap();
    let second = document.add_geometry(point(2.)).unwrap();
    document.commit_group_transaction(&mut group).unwrap();
    document.objects[0].attributes =
        ObjectAttributes::on_layer(document.current_layer_id()).with_name("corrupt history");
    document
        .select_object(second, SelectionMode::Replace)
        .unwrap();
    let before = format!("{document:?}");
    let checkpoint = format!("{group:?}");
    assert!(matches!(
        document.undo_history_group(&mut group),
        Err(DocumentError::HistoryInvariant(_))
    ));
    assert_eq!(format!("{document:?}"), before);
    assert_eq!(format!("{group:?}"), checkpoint);
}
