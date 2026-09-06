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
            (vec![(1, ids[1]), (0, ids[0])], 4),
            (vec![(0, ids[3]), (0, ids[3])], 4),
            (vec![(1, ids[0])], 4),
        ] {
            assert!(apply(&mut d.objects, &moved, count, forward).is_err());
            assert_eq!(d.objects, before);
        }
    }
}
