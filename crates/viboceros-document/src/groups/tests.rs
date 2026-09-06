use super::*;
use viboceros_geometry::{Point3, Vector3};

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
