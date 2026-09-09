use super::*;

fn fixture() -> (Document, Vec<ObjectId>) {
    let mut document = Document::default();
    let ids = (0..6)
        .map(|i| {
            document
                .add_geometry(Geometry::Point(
                    Point3::try_new(i as f64, 0.0, 0.0).unwrap(),
                ))
                .unwrap()
        })
        .collect::<Vec<_>>();
    document
        .add_group(Some("all".into()), ids.iter().copied())
        .unwrap();
    document
        .add_group(Some("even".into()), [ids[0], ids[2], ids[4]])
        .unwrap();
    for i in [5, 2, 0, 4, 1, 3] {
        document
            .select_objects_direct([ids[i]], SelectionMode::Add)
            .unwrap();
    }
    (document, ids)
}

#[test]
fn all_subsets_restore_object_order_groups_and_selection_membership() {
    for mask in 1..64 {
        let (mut document, ids) = fixture();
        let before_objects = document.objects.clone();
        let before_groups = document.groups.clone();
        let removed = ids
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, id)| *id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            document
                .delete_objects(removed.iter().copied().chain(removed.iter().copied()))
                .unwrap(),
            removed.len()
        );
        let after_objects = document.objects.clone();
        let after_groups = document.groups.clone();
        let mut expected_groups = before_groups.clone();
        for group in &mut expected_groups {
            group.members.retain(|id| !removed.contains(id));
        }
        assert_eq!(after_groups, expected_groups);
        assert_eq!(
            after_objects,
            before_objects
                .iter()
                .filter(|o| !removed.contains(&o.id))
                .cloned()
                .collect::<Vec<_>>()
        );
        for group in document.groups() {
            assert!(group.members.iter().all(|id| !removed.contains(id)));
        }
        for _ in 0..2 {
            document.undo().unwrap();
            assert_eq!(document.objects, before_objects, "mask={mask}");
            assert_eq!(document.groups, before_groups, "mask={mask}");
            assert_eq!(document.selection, ids.iter().copied().collect());
            document.redo().unwrap();
            assert_eq!(document.objects, after_objects);
            assert_eq!(document.groups, after_groups);
            assert_eq!(
                document.selection,
                ids.iter()
                    .copied()
                    .filter(|id| !removed.contains(id))
                    .collect()
            );
        }
    }
}

#[test]
fn empty_and_missing_ids_preserve_redo_and_all_document_state() {
    let (mut document, ids) = fixture();
    document.delete_objects([ids[0]]).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    assert_eq!(document.delete_objects([]).unwrap(), 0);
    assert!(matches!(
        document.delete_objects([ids[1], ObjectId::new()]),
        Err(DocumentError::ObjectNotFound(_))
    ));
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn transaction_rollback_restores_pick_order_and_interleaved_edits() {
    let (mut document, ids) = fixture();
    let before_objects = document.objects.clone();
    let before_groups = document.groups.clone();
    let before_selection = document.selection_order.clone();
    document.begin_transaction("mixed").unwrap();
    document.delete_objects([ids[0], ids[3], ids[5]]).unwrap();
    document
        .add_geometry(Geometry::Point(Point3::try_new(99.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document.delete_objects([ids[1], ids[4]]).unwrap();
    document.clear_selection();
    document.rollback_transaction().unwrap();
    assert_eq!(document.objects, before_objects);
    assert_eq!(document.groups, before_groups);
    assert_eq!(document.selection_order, before_selection);
}

#[test]
fn replay_exchanges_removed_selection_without_replacing_unrelated_choices() {
    let (mut document, ids) = fixture();
    document.clear_selection();
    document
        .select_objects_direct([ids[2]], SelectionMode::Add)
        .unwrap();
    document.delete_objects([ids[2], ids[4]]).unwrap();
    document
        .select_objects_direct([ids[1]], SelectionMode::Add)
        .unwrap();
    document.undo().unwrap();
    assert_eq!(document.selection, BTreeSet::from([ids[1], ids[2]]));
    document
        .select_objects_direct([ids[2]], SelectionMode::Remove)
        .unwrap();
    document
        .select_objects_direct([ids[4]], SelectionMode::Add)
        .unwrap();
    document.redo().unwrap();
    assert_eq!(document.selection, BTreeSet::from([ids[1]]));
    document.undo().unwrap();
    assert_eq!(document.selection, BTreeSet::from([ids[1], ids[4]]));
}

#[test]
fn multiple_batches_replay_around_object_and_group_creation() {
    let (mut document, ids) = fixture();
    let before_objects = document.objects.clone();
    let before_groups = document.groups.clone();
    document.begin_transaction("mixed batch").unwrap();
    document.delete_objects([ids[1], ids[4]]).unwrap();
    let fresh = document
        .add_geometry(Geometry::Point(Point3::try_new(99.0, 0.0, 0.0).unwrap()))
        .unwrap();
    document
        .add_group(Some("fresh group".into()), [fresh, ids[0]])
        .unwrap();
    document.delete_objects([fresh, ids[3]]).unwrap();
    document.commit_transaction().unwrap();
    let after_objects = document.objects.clone();
    let after_groups = document.groups.clone();
    document.undo().unwrap();
    assert_eq!(document.objects, before_objects);
    assert_eq!(document.groups, before_groups);
    document.redo().unwrap();
    assert_eq!(document.objects, after_objects);
    assert_eq!(document.groups, after_groups);
}
