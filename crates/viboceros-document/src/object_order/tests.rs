use super::*;

fn fixture(count: usize) -> (Document, Vec<ObjectId>) {
    let mut d = Document::default();
    let ids = (0..count)
        .map(|i| {
            d.add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    d.add_group(Some("all".into()), ids.iter().copied())
        .unwrap();
    for id in ids.iter().rev() {
        d.select_objects_direct([*id], SelectionMode::Add).unwrap();
    }
    (d, ids)
}

#[test]
fn every_subset_preserves_relative_order_and_roundtrips_without_changing_objects() {
    for mask in 0..256 {
        let (mut d, ids) = fixture(8);
        let before = d.objects().cloned().collect::<Vec<_>>();
        let groups = d.groups().cloned().collect::<Vec<_>>();
        let moved = ids
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, id)| *id)
            .collect::<Vec<_>>();
        let expected = before
            .iter()
            .filter(|o| !moved.contains(&o.id))
            .chain(before.iter().filter(|o| moved.contains(&o.id)))
            .cloned()
            .collect::<Vec<_>>();
        let history = d.undo_label().map(str::to_owned);
        assert_eq!(
            d.move_objects_to_end(moved.iter().rev().copied()).unwrap(),
            expected != before
        );
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), expected);
        if expected != before {
            for _ in 0..3 {
                d.undo().unwrap();
                assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
                d.redo().unwrap();
                assert_eq!(d.objects().cloned().collect::<Vec<_>>(), expected);
            }
        } else {
            assert_eq!(d.undo_label(), history.as_deref());
        }
        assert_eq!(d.groups().cloned().collect::<Vec<_>>(), groups);
        assert_eq!(
            d.selected_object_ids().collect::<Vec<_>>(),
            ids.iter().rev().copied().collect::<Vec<_>>()
        );
    }
}

#[test]
fn ordering_composes_with_insert_remove_geometry_edits_and_transaction_rollback() {
    let (mut d, ids) = fixture(5);
    let before = d.objects().cloned().collect::<Vec<_>>();
    let groups = d.groups().cloned().collect::<Vec<_>>();
    for rollback in [true, false] {
        d.begin_transaction("batch").unwrap();
        d.replace_object_geometries([(
            ids[0],
            Geometry::Point(Point3::try_new(99., 0., 0.).unwrap()),
        )])
        .unwrap();
        d.move_objects_to_end([ids[0], ids[3]]).unwrap();
        d.delete_object(ids[2]).unwrap();
        d.add_geometry(Geometry::Point(Point3::try_new(100., 0., 0.).unwrap()))
            .unwrap();
        d.move_objects_to_end([ids[1]]).unwrap();
        if rollback {
            d.rollback_transaction().unwrap();
        } else {
            d.commit_transaction().unwrap();
            let after = d.objects().cloned().collect::<Vec<_>>();
            for _ in 0..3 {
                d.undo().unwrap();
                assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
                d.redo().unwrap();
                assert_eq!(d.objects().cloned().collect::<Vec<_>>(), after);
            }
            d.undo().unwrap();
        }
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(d.groups().cloned().collect::<Vec<_>>(), groups);
    }
}

#[test]
fn missing_ids_and_noop_order_do_not_mutate_or_clear_redo() {
    let (mut d, ids) = fixture(4);
    d.move_objects_to_end([ids[0]]).unwrap();
    d.undo().unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    let missing = ObjectId::new();
    assert_eq!(
        d.move_objects_to_end([ids[0], missing]),
        Err(DocumentError::ObjectNotFound(missing))
    );
    assert!(!d.move_objects_to_end([]).unwrap());
    assert!(!d.move_objects_to_end([ids[3], ids[3]]).unwrap());
    assert!(d.can_redo());
    assert_eq!(d.undo_label(), history.as_deref());
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn corrupted_order_plans_fail_before_swapping_any_object() {
    let (mut d, ids) = fixture(4);
    let before = d.objects.clone();
    for forward in [false, true] {
        for (moved, count) in [
            (vec![(0, ids[0])], 5),
            (vec![(4, ids[0])], 4),
            (vec![(1, ids[1]), (0, ids[2])], 4),
            (vec![(0, ids[3]), (0, ids[3])], 4),
            (vec![(1, ids[0])], 4),
        ] {
            assert!(apply(&mut d.objects, &moved, count, forward).is_err());
            assert_eq!(d.objects, before);
        }
    }
}

#[test]
fn every_ordered_subset_roundtrips_and_retains_all_nonordering_state() {
    fn visit(picks: &mut Vec<usize>, remaining: &mut Vec<usize>) {
        let (mut document, ids) = fixture(5);
        let before = document.objects().cloned().collect::<Vec<_>>();
        let groups = document.groups().cloned().collect::<Vec<_>>();
        let expected = (0..ids.len())
            .filter(|i| !picks.contains(i))
            .chain(picks.iter().copied())
            .map(|i| before[i].clone())
            .collect::<Vec<_>>();
        let history = document.undo_label().map(str::to_owned);
        assert_eq!(
            document
                .move_objects_to_end_in_order(picks.iter().map(|i| ids[*i]))
                .unwrap(),
            expected != before
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), expected);
        if expected != before {
            for _ in 0..3 {
                document.undo().unwrap();
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
                document.redo().unwrap();
                assert_eq!(document.objects().cloned().collect::<Vec<_>>(), expected);
            }
        } else {
            assert_eq!(document.undo_label(), history.as_deref());
        }
        assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            ids.iter().rev().copied().collect::<Vec<_>>()
        );
        for i in 0..remaining.len() {
            let choice = remaining.remove(i);
            picks.push(choice);
            visit(picks, remaining);
            picks.pop();
            remaining.insert(i, choice);
        }
    }
    visit(&mut vec![], &mut (0..5).collect());
}

#[test]
fn ordered_moves_coalesce_duplicates_and_fail_atomically_on_missing_ids() {
    let (mut document, ids) = fixture(4);
    let before = document.objects().cloned().collect::<Vec<_>>();
    document
        .move_objects_to_end_in_order([ids[2], ids[0], ids[2]])
        .unwrap();
    assert_eq!(
        document.objects().map(|o| o.id()).collect::<Vec<_>>(),
        [ids[1], ids[3], ids[2], ids[0]]
    );
    document.undo().unwrap();
    let missing = ObjectId::new();
    assert_eq!(
        document.move_objects_to_end_in_order([ids[0], missing]),
        Err(DocumentError::ObjectNotFound(missing))
    );
    assert!(
        !document
            .move_objects_to_end_in_order([ids[2], ids[3]])
            .unwrap()
    );
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(document.can_redo());
    document.begin_transaction("ordered rollback").unwrap();
    document
        .move_objects_to_end_in_order(ids.iter().rev().copied())
        .unwrap();
    document.delete_object(ids[2]).unwrap();
    document
        .add_geometry(Geometry::Point(Point3::try_new(100., 0., 0.).unwrap()))
        .unwrap();
    document.rollback_transaction().unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn ordered_geometry_copies_preserve_attributes_memberships_and_atomic_history() {
    let (mut document, ids) = fixture(4);
    let before = document.objects().cloned().collect::<Vec<_>>();
    let groups = document.groups().cloned().collect::<Vec<_>>();
    let geometry = |x| Geometry::Point(Point3::try_new(x, 10., 0.).unwrap());
    let copied = document
        .copy_object_geometries_into_source_groups_in_order([
            (ids[2], geometry(102.)),
            (ids[0], geometry(100.)),
            (ids[2], geometry(202.)),
        ])
        .unwrap();
    assert_eq!(copied.len(), 2);
    for (index, source, expected) in [(0, ids[0], geometry(100.)), (1, ids[2], geometry(202.))] {
        let output = document.object(copied[index]).unwrap();
        let source = document.object(source).unwrap();
        assert_eq!(output.geometry(), &expected);
        assert_eq!(output.attributes(), source.attributes());
        assert_eq!(output.group_ids(), source.group_ids());
        assert!(!document.is_selected(output.id()));
    }
    assert_eq!(
        document
            .objects()
            .skip(4)
            .map(|o| o.id())
            .collect::<Vec<_>>(),
        copied
    );
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        ids.iter().rev().copied().collect::<Vec<_>>()
    );
    let after = document.objects().cloned().collect::<Vec<_>>();
    for _ in 0..3 {
        document.undo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.groups().cloned().collect::<Vec<_>>(), groups);
        document.redo().unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    }
    document.undo().unwrap();
    let missing = ObjectId::new();
    assert_eq!(
        document.copy_object_geometries_into_source_groups_in_order([
            (ids[0], geometry(100.)),
            (missing, geometry(200.))
        ]),
        Err(DocumentError::ObjectNotFound(missing))
    );
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(document.can_redo());
}
