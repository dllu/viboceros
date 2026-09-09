use super::*;
use viboceros_geometry::{Point3, Vector3};

#[test]
fn batch_pop_groups_uses_each_objects_last_membership_and_replays_exactly() {
    for mask in 0_u8..8 {
        let (mut document, ids, _) = fixture();
        let before = state(&document);
        let request = (0..3)
            .rev()
            .filter(|i| mask & (1 << i) != 0)
            .flat_map(|i| [ids[i], ids[i]])
            .collect::<Vec<_>>();
        assert_eq!(
            document
                .pop_object_group_memberships(request.clone())
                .unwrap(),
            mask.count_ones() as usize
        );
        for (i, id) in ids.iter().enumerate() {
            let mut expected = before.0[i].clone();
            if mask & (1 << i) != 0 {
                expected.group_ids.pop();
            }
            assert_eq!(document.object(*id).unwrap(), &expected);
        }
        let after = state(&document);
        assert_eq!(before.1.len(), after.1.len());
        if mask != 0 {
            document.undo().unwrap();
            assert_eq!(state(&document), before);
            document.redo().unwrap();
            assert_eq!(state(&document), after);
        }
        document.clear_object_group_memberships(ids).unwrap();
        document.add_geometry(geometry(99.)).unwrap();
        document.undo().unwrap();
        let debug = format!("{document:?}");
        assert_eq!(document.pop_object_group_memberships(request).unwrap(), 0);
        assert_eq!(format!("{document:?}"), debug);
    }
}

#[test]
fn group_batches_reject_late_membership_corruption_before_any_edit() {
    for operation in ["create", "append", "remove", "pop"] {
        for active in [false, true] {
            let (mut document, ids, groups) = fixture();
            let target = document.add_empty_group(None).unwrap();
            document.add_geometry(geometry(99.)).unwrap();
            document.undo().unwrap();
            // The final object's unrelated group is broken: earlier objects
            // must not be edited before this failure is discovered.
            document
                .groups
                .iter_mut()
                .find(|g| g.id == groups[1])
                .unwrap()
                .members
                .remove(&ids[2]);
            if active {
                document.begin_transaction("caller").unwrap();
            }
            let before = format!("{document:?}");
            let result = match operation {
                "create" => document.add_group(None, ids).map(|_| ()),
                "append" => document.add_group_members(target, ids).map(|_| ()),
                "remove" => document.remove_group(groups[0]).map(|_| ()),
                "pop" => document.pop_object_group_memberships(ids).map(|_| ()),
                _ => unreachable!(),
            };
            assert!(result.is_err(), "{operation}, active={active}");
            assert_eq!(
                format!("{document:?}"),
                before,
                "{operation}, active={active}"
            );
        }
    }
}

#[test]
fn adding_an_indexed_member_does_not_hide_a_missing_forward_membership() {
    let (mut document, ids, groups) = fixture();
    document.objects[2].group_ids.retain(|id| *id != groups[0]);
    let before = format!("{document:?}");
    assert!(document.add_group_members(groups[0], [ids[2]]).is_err());
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn batch_clear_groups_preserves_peers_definitions_and_history() {
    let (mut document, ids, _) = fixture();
    let original = state(&document);
    assert_eq!(
        document
            .clear_object_group_memberships([ids[2], ids[0], ids[2]])
            .unwrap(),
        2
    );
    assert!(document.object(ids[0]).unwrap().group_ids().is_empty());
    assert!(document.object(ids[2]).unwrap().group_ids().is_empty());
    assert_eq!(document.object(ids[1]).unwrap(), &original.0[1]);
    assert_eq!(document.groups().len(), original.1.len());
    consistent(&document);
    let changed = state(&document);
    document.undo().unwrap();
    assert_eq!(state(&document), original);
    document.redo().unwrap();
    assert_eq!(state(&document), changed);
    document.add_geometry(geometry(99.)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    assert_eq!(
        document
            .clear_object_group_memberships([ids[0], ids[2]])
            .unwrap(),
        0
    );
    assert_eq!(document.clear_object_group_memberships([]).unwrap(), 0);
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn batch_clear_groups_preflights_all_objects_even_in_a_transaction() {
    for active in [false, true] {
        for corrupt in [false, true] {
            let (mut document, ids, groups) = fixture();
            document.add_geometry(geometry(99.)).unwrap();
            document.undo().unwrap();
            if corrupt {
                document
                    .groups
                    .iter_mut()
                    .find(|g| g.id == groups[0])
                    .unwrap()
                    .members
                    .remove(&ids[2]);
            }
            if active {
                document.begin_transaction("caller").unwrap();
            }
            let before = format!("{document:?}");
            let requested = if corrupt {
                ids.to_vec()
            } else {
                vec![ids[0], ObjectId::new()]
            };
            assert!(document.clear_object_group_memberships(requested).is_err());
            assert_eq!(format!("{document:?}"), before);
        }
    }
}

#[test]
fn batch_clear_groups_joins_and_rolls_back_with_the_caller() {
    let (mut document, ids, _) = fixture();
    document.add_geometry(geometry(99.)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    document.begin_transaction("caller").unwrap();
    document.add_geometry(geometry(100.)).unwrap();
    assert_eq!(document.clear_object_group_memberships(ids).unwrap(), 3);
    assert!(
        document
            .objects()
            .all(|object| object.group_ids().is_empty())
    );
    assert!(document.groups().all(|group| group.members().len() == 0));
    document.rollback_transaction().unwrap();
    assert_eq!(format!("{document:?}"), before);
}

#[test]
#[ignore = "manual large group creation timing"]
fn benchmark_large_group_creation() {
    let mut document = Document::default();
    document.begin_transaction("fixture").unwrap();
    let ids = (0..20_000)
        .map(|i| document.add_geometry(geometry(i as f64)).unwrap())
        .collect::<Vec<_>>();
    document.commit_transaction().unwrap();
    let start = std::time::Instant::now();
    let first = document.add_group(None, ids.iter().rev().copied()).unwrap();
    eprintln!("20k points, create group: {:?}", start.elapsed());
    let second = document.add_empty_group(None).unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        document
            .add_group_members(second, ids.iter().rev().copied())
            .unwrap(),
        ids.len()
    );
    eprintln!("20k points, add group members: {:?}", start.elapsed());
    assert_eq!(
        document.group(first).unwrap().members,
        ids.iter().copied().collect()
    );
    assert_eq!(
        document.group(second).unwrap().members,
        ids.iter().copied().collect()
    );
    assert!(
        document
            .objects
            .iter()
            .all(|object| object.group_ids == [first, second])
    );
    consistent(&document);
    let start = std::time::Instant::now();
    assert_eq!(document.remove_group(first).unwrap(), ids.len());
    eprintln!("20k points, remove group: {:?}", start.elapsed());
    assert!(
        document
            .objects
            .iter()
            .all(|object| object.group_ids == [second])
    );
    consistent(&document);
    let start = std::time::Instant::now();
    assert_eq!(
        document
            .clear_object_group_memberships(ids.iter().rev().copied())
            .unwrap(),
        ids.len()
    );
    eprintln!("20k points, clear memberships: {:?}", start.elapsed());
    assert!(
        document
            .objects()
            .all(|object| object.group_ids().is_empty())
    );
    assert!(document.groups().all(|group| group.members().len() == 0));
    consistent(&document);
}

#[test]
#[ignore = "manual grouped copy timing"]
fn benchmark_large_group_copy() {
    let mut document = Document::default();
    document.begin_transaction("fixture").unwrap();
    let ids = (0..20_000)
        .map(|i| {
            document
                .add_geometry(Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap()))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let group = document.add_empty_group(None).unwrap();
    for object in &mut document.objects {
        object.group_ids.push(group);
    }
    document.groups[0].members.extend(ids.iter().copied());
    document.commit_transaction().unwrap();
    let start = std::time::Instant::now();
    let copies = document
        .copy_objects_transformed(ids, AffineTransform3::identity())
        .unwrap();
    eprintln!("20k grouped points, copy: {:?}", start.elapsed());
    assert_eq!(copies.len(), 20_000);
    let copied_group = document.groups[1].id;
    assert_eq!(document.groups[1].members, copies.iter().copied().collect());
    for (i, object) in document.objects.iter().skip(20_000).enumerate() {
        assert_eq!(object.group_ids, [copied_group]);
        assert_eq!(
            object.geometry,
            Geometry::Point(Point3::try_new(i as f64, 0., 0.).unwrap())
        );
    }
}

fn geometry(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}

fn fixture() -> (Document, [ObjectId; 3], [GroupId; 3]) {
    let mut document = Document::default();
    let objects = std::array::from_fn(|i| document.add_geometry(geometry(i as f64)).unwrap());
    let groups = std::array::from_fn(|i| {
        document
            .add_empty_group(Some(format!("Group-{i}")))
            .unwrap()
    });
    for (id, order) in objects.into_iter().zip([[2, 0, 1], [1, 2, 0], [0, 1, 2]]) {
        document
            .set_object_group_memberships(id, order.map(|i| groups[i]))
            .unwrap();
    }
    document.select_all();
    consistent(&document);
    (document, objects, groups)
}

fn consistent(document: &Document) {
    for object in document.objects() {
        assert_eq!(
            object.group_ids().iter().collect::<BTreeSet<_>>().len(),
            object.group_ids().len()
        );
        assert_eq!(object.top_group(), object.group_ids().last().copied());
        for id in object.group_ids() {
            assert!(document.group(*id).unwrap().members.contains(&object.id()));
        }
    }
    for group in document.groups() {
        for id in group.members() {
            assert!(
                document
                    .object(id)
                    .unwrap()
                    .group_ids()
                    .contains(&group.id())
            );
        }
    }
}

fn state(document: &Document) -> (Vec<Object>, Vec<Group>) {
    consistent(document);
    (
        document.objects().cloned().collect(),
        document.groups().cloned().collect(),
    )
}

#[test]
fn group_removal_rejects_broken_member_indexes_before_transaction_edits() {
    for active in [false, true] {
        for corruption in 0..3 {
            let (mut document, ids, groups) = fixture();
            document.add_geometry(geometry(99.)).unwrap();
            document.undo().unwrap();
            if active {
                document.begin_transaction("caller").unwrap();
            }
            let expected = match corruption {
                0 => {
                    let missing = ObjectId::new();
                    document.groups[0].members.insert(missing);
                    DocumentError::ObjectNotFound(missing)
                }
                1 => {
                    document.groups[0].members.remove(&ids[0]);
                    DocumentError::HistoryInvariant("group member index does not match")
                }
                _ => {
                    document.objects[0].group_ids.retain(|id| *id != groups[0]);
                    DocumentError::HistoryInvariant("group member index does not match")
                }
            };
            let before = format!("{document:?}");
            assert_eq!(document.remove_group(groups[0]), Err(expected));
            assert_eq!(format!("{document:?}"), before);
        }
    }
}

#[test]
fn empty_group_removal_preserves_objects_and_replays_its_table_position() {
    let (mut document, _, _) = fixture();
    let empty = document.add_empty_group(Some("unused".into())).unwrap();
    document.add_empty_group(Some("later".into())).unwrap();
    let before = state(&document);
    let selection = document.selection_order.clone();
    assert_eq!(document.remove_group(empty).unwrap(), 0);
    assert_eq!(document.objects, before.0);
    assert_eq!(document.selection_order, selection);
    let after = state(&document);
    document.undo().unwrap();
    assert_eq!(state(&document), before);
    document.redo().unwrap();
    assert_eq!(state(&document), after);
}

#[test]
fn batch_group_additions_preserve_order_noops_and_exact_history() {
    for create in [false, true] {
        for mask in 0_u8..8 {
            let (mut document, ids, _) = fixture();
            let target = if create {
                None
            } else {
                Some(document.add_group(None, [ids[0]]).unwrap())
            };
            let original = state(&document);
            let original_selection = document.selection_order.clone();
            let original_debug = format!("{document:?}");
            let request = (0..3)
                .rev()
                .filter(|i| mask & (1 << i) != 0)
                .flat_map(|i| [ids[i], ids[i]])
                .collect::<Vec<_>>();
            if create && mask == 0 {
                assert_eq!(
                    document.add_group(None, request),
                    Err(DocumentError::EmptyGroup)
                );
                assert_eq!(format!("{document:?}"), original_debug);
                continue;
            }
            let (group, changed) = if let Some(target) = target {
                let changed = document.add_group_members(target, request).unwrap();
                assert_eq!(changed, (mask & !1).count_ones() as usize);
                (target, changed)
            } else {
                (
                    document.add_group(None, request).unwrap(),
                    mask.count_ones() as usize,
                )
            };
            if changed == 0 {
                assert_eq!(format!("{document:?}"), original_debug);
                continue;
            }
            for (i, object) in document.objects.iter().enumerate() {
                let mut expected = original.0[i].group_ids.clone();
                if mask & (1 << i) != 0 && !expected.contains(&group) {
                    expected.push(group);
                }
                assert_eq!(object.group_ids, expected);
            }
            assert_eq!(document.selection_order, original_selection);
            let changed_state = state(&document);
            document.undo().unwrap();
            assert_eq!(state(&document), original);
            document.redo().unwrap();
            assert_eq!(state(&document), changed_state);
        }
    }
}

#[test]
fn missing_group_sources_preserve_redo_and_existing_transaction() {
    for create in [false, true] {
        for active in [false, true] {
            let (mut document, ids, groups) = fixture();
            document.add_geometry(geometry(99.)).unwrap();
            document.undo().unwrap();
            if active {
                document.begin_transaction("caller").unwrap();
            }
            let before = format!("{document:?}");
            let missing = [ObjectId::new(), ObjectId::new()];
            let request = ids.into_iter().chain(missing).collect::<Vec<_>>();
            let result = if create {
                document.add_group(None, request).map(|_| 0)
            } else {
                document.add_group_members(groups[0], request)
            };
            assert_eq!(
                result,
                Err(DocumentError::ObjectNotFound(
                    *missing.iter().min().unwrap()
                ))
            );
            assert_eq!(format!("{document:?}"), before);
            if active {
                document.rollback_transaction().unwrap();
            }
        }
    }
}

#[test]
fn indexed_membership_transition_rejects_corruption_without_partial_changes() {
    for failure in 0..4 {
        let (mut document, ids, groups) = fixture();
        let index = 1;
        let mut before = document.objects[index].group_ids.clone();
        let mut after = vec![groups[0], groups[2]];
        let expected = match failure {
            0 => {
                before.reverse();
                "ordered object memberships do not match"
            }
            1 => {
                after.push(groups[0]);
                "duplicate ordered membership"
            }
            2 => {
                after.push(GroupId::new());
                "membership group is missing"
            }
            _ => {
                document.groups[2].members.remove(&ids[index]);
                "group member index does not match"
            }
        };
        let original = format!("{document:?}");
        assert_eq!(
            apply_memberships_at(&mut document, index, &before, &after),
            Err(DocumentError::HistoryInvariant(expected))
        );
        assert_eq!(format!("{document:?}"), original);
        assert_eq!(
            apply_memberships(&mut document, ids[index], &before, &after),
            Err(DocumentError::HistoryInvariant(expected))
        );
        assert_eq!(format!("{document:?}"), original);
    }
}

#[test]
fn membership_order_is_per_object_not_group_table_order() {
    let (mut document, ids, groups) = fixture();
    let original = state(&document);
    assert_eq!(
        document.object(ids[0]).unwrap().group_ids(),
        &[groups[2], groups[0], groups[1]]
    );
    assert_eq!(
        document.object(ids[1]).unwrap().top_group(),
        Some(groups[0])
    );
    let undo = document.undo_label().map(str::to_owned);
    assert_eq!(
        document
            .add_group_members(groups[2], [ids[0], ids[0]])
            .unwrap(),
        0
    );
    assert_eq!(document.undo_label(), undo.as_deref());
    assert_eq!(state(&document), original);
    document
        .set_object_group_memberships(ids[0], [groups[1], groups[0], groups[2]])
        .unwrap();
    assert_eq!(
        document.object(ids[0]).unwrap().top_group(),
        Some(groups[2])
    );
    assert_eq!(document.groups().cloned().collect::<Vec<_>>(), original.1);
    let changed = state(&document);
    document.undo().unwrap();
    assert_eq!(state(&document), original);
    document.redo().unwrap();
    assert_eq!(state(&document), changed);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), ids);
}

#[test]
fn adding_to_an_older_group_appends_and_undo_remembers_the_prior_top_group() {
    let mut document = Document::default();
    let a = document.add_geometry(geometry(0.)).unwrap();
    let b = document.add_geometry(geometry(1.)).unwrap();
    let old = document.add_group(Some("Old".into()), [a]).unwrap();
    let new = document.add_group(Some("New".into()), [a, b]).unwrap();
    document.add_group_members(old, [b]).unwrap();
    assert_eq!(document.object(a).unwrap().group_ids(), &[old, new]);
    assert_eq!(document.object(b).unwrap().group_ids(), &[new, old]);
    consistent(&document);
    document.undo().unwrap();
    assert_eq!(document.object(b).unwrap().group_ids(), &[new]);
    document.redo().unwrap();
    assert_eq!(document.object(b).unwrap().group_ids(), &[new, old]);
}

#[test]
fn removing_definitions_and_deleting_members_restore_order_on_replay() {
    for removed in 0..3 {
        let (mut document, ids, groups) = fixture();
        let original = state(&document);
        assert_eq!(document.remove_group(groups[removed]).unwrap(), 3);
        for (object, before) in document.objects().zip(&original.0) {
            assert_eq!(
                object.group_ids(),
                before
                    .group_ids()
                    .iter()
                    .copied()
                    .filter(|id| *id != groups[removed])
                    .collect::<Vec<_>>()
            );
        }
        let changed = state(&document);
        document.undo().unwrap();
        assert_eq!(state(&document), original);
        document.redo().unwrap();
        assert_eq!(state(&document), changed);
        document.undo().unwrap();
        document.begin_transaction("Delete all members").unwrap();
        for id in [ids[1], ids[0], ids[2]] {
            document.delete_object(id).unwrap();
            consistent(&document);
        }
        document.commit_transaction().unwrap();
        assert_eq!(document.groups().len(), 3);
        assert!(document.groups().all(|g| g.members().len() == 0));
        document.undo().unwrap();
        assert_eq!(state(&document), original);
        document.redo().unwrap();
        assert_eq!(document.objects().len(), 0);
        document.undo().unwrap();
        assert_eq!(state(&document), original);
    }
}

#[test]
fn group_edits_geometry_edits_and_clear_are_atomic_together() {
    let (mut document, ids, groups) = fixture();
    let original = state(&document);
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    document.begin_transaction("Mixed edits").unwrap();
    document
        .set_object_group_memberships(ids[0], [groups[0]])
        .unwrap();
    document
        .replace_object_geometries([(ids[0], geometry(10.))])
        .unwrap();
    document.remove_group(groups[1]).unwrap();
    document.delete_object(ids[2]).unwrap();
    document
        .add_group(Some("Temporary".into()), [ids[1]])
        .unwrap();
    document.clear_objects();
    document.rollback_transaction().unwrap();
    assert_eq!(state(&document), original);
    assert_eq!(
        document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    document.clear_objects();
    document.undo().unwrap();
    assert_eq!(state(&document), original);
    document.redo().unwrap();
    assert_eq!(document.objects().len(), 0);
    assert_eq!(document.groups().len(), 0);
}

#[test]
fn invalid_memberships_are_validated_before_edits_or_redo_loss() {
    let (mut document, ids, groups) = fixture();
    document.set_object_group_memberships(ids[0], []).unwrap();
    document.undo().unwrap();
    let original = state(&document);
    let redo = document.redo_label().map(str::to_owned);
    for memberships in [
        vec![groups[0], groups[0]],
        vec![GroupId::new()],
        vec![groups[1], GroupId::new()],
    ] {
        assert!(
            document
                .set_object_group_memberships(ids[0], memberships)
                .is_err()
        );
        assert_eq!(state(&document), original);
        assert_eq!(document.redo_label(), redo.as_deref());
    }
    assert!(
        document
            .add_group_members(groups[0], [ids[0], ObjectId::new()])
            .is_err()
    );
    assert!(
        document
            .add_group(Some("Group-0".into()), [ids[0]])
            .is_err()
    );
    assert_eq!(state(&document), original);
    assert_eq!(document.redo_label(), redo.as_deref());
}

struct Shift;
impl PointMorph for Shift {
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        point.translated(Vector3::try_new(4., 2., 1.)?)
    }
}

#[test]
fn all_copy_paths_preserve_each_objects_membership_order() {
    for mode in [
        "affine",
        "morph",
        "layer",
        "same-groups",
        "omit",
        "definitions",
    ] {
        let (mut document, ids, groups) = fixture();
        let layer = document.add_layer("Destination", ColorRgb::BLACK).unwrap();
        let original = state(&document);
        let copied = match mode {
            "affine" => document
                .copy_objects_with_transforms(ids, &[AffineTransform3::identity(); 2])
                .unwrap(),
            "morph" => document.copy_objects_morphed(ids, &Shift).unwrap(),
            "layer" => document.copy_objects_to_layer(ids, layer).unwrap(),
            "same-groups" => document
                .copy_object_geometries_into_source_groups(
                    ids.into_iter()
                        .enumerate()
                        .map(|(i, id)| (id, geometry(i as f64 + 10.))),
                )
                .unwrap(),
            "omit" | "definitions" => document
                .copy_objects_with_transforms_and_groups(
                    ids,
                    &[AffineTransform3::identity()],
                    if mode == "omit" {
                        CopyGroupPolicy::Omit
                    } else {
                        CopyGroupPolicy::DefinitionsOnly
                    },
                )
                .unwrap(),
            _ => unreachable!(),
        };
        for (instance, copies) in copied.chunks(3).enumerate() {
            let copied_groups = if mode == "same-groups" {
                groups.to_vec()
            } else {
                document
                    .groups()
                    .skip(3 + instance * 3)
                    .take(3)
                    .map(Group::id)
                    .collect::<Vec<_>>()
            };
            for (copy, order) in copies.iter().zip([[2, 0, 1], [1, 2, 0], [0, 1, 2]]) {
                assert_eq!(
                    document.object(*copy).unwrap().group_ids(),
                    if matches!(mode, "omit" | "definitions") {
                        vec![]
                    } else {
                        // First source's [2,0,1] order allocates new definitions
                        // in that order; original group indices map to [1,2,0].
                        order
                            .map(|i| {
                                copied_groups[if mode == "same-groups" {
                                    i
                                } else {
                                    [1, 2, 0][i]
                                }]
                            })
                            .to_vec()
                    },
                    "{mode}"
                );
            }
        }
        let changed = state(&document);
        if mode == "definitions" {
            assert_eq!(document.groups().len(), 6);
            assert!(
                document
                    .groups()
                    .skip(3)
                    .all(|group| group.members().len() == 0)
            );
        }
        for _ in 0..3 {
            document.undo().unwrap();
            assert_eq!(state(&document), original, "{mode}");
            document.redo().unwrap();
            assert_eq!(state(&document), changed, "{mode}");
        }
    }
}
