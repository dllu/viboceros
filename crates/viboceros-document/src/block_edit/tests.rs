use super::*;
use viboceros_geometry::Point3;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn fixture() -> (Document, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let source = doc.add_geometry(Geometry::Point(p(1., 2., 3.))).unwrap();
    let (definition, peer) = doc
        .create_block_from_objects("part", p(0., 0., 0.), [source])
        .unwrap();
    let root = doc
        .add_block_instance(
            BlockReference::try_new(
                definition,
                AffineTransform3::from_translation(Vector3::try_new(10., 20., 30.).unwrap()),
            )
            .unwrap(),
        )
        .unwrap();
    (doc, peer, root)
}
#[test]
fn editing_members_saves_one_model_transaction_and_restores_all_roots() {
    let (mut doc, peer, root) = fixture();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let members = doc.open_block_edit(root).unwrap();
    assert_eq!(members.len(), 1);
    assert!(!doc.is_object_selectable(peer));
    doc.transform_objects(
        members,
        AffineTransform3::from_translation(Vector3::try_new(2., -3., 4.).unwrap()),
    )
    .unwrap();
    assert_eq!(doc.save_block_edit().unwrap(), 1);
    assert!(!doc.is_block_editing());
    let Geometry::BlockInstance(i) = doc.object(peer).unwrap().geometry() else {
        panic!()
    };
    assert!(matches!(&*i.members()[0].geometry,Geometry::Point(point)if *point==p(3.,-1.,7.)));
    assert_eq!(doc.undo_label(), Some("BlockEdit"));
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}
#[test]
fn cancel_restores_exact_document_and_redo_including_created_geometry() {
    let (mut doc, _, root) = fixture();
    doc.add_geometry(Geometry::Point(p(7., 0., 0.))).unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let ids = doc.open_block_edit(root).unwrap();
    doc.delete_objects(ids).unwrap();
    doc.add_geometry(Geometry::Point(p(3., 4., 5.))).unwrap();
    doc.discard_block_edit().unwrap();
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
#[test]
fn nonuniform_placement_and_nested_edit_reject_without_mutation() {
    let (mut doc, _, root) = fixture();
    let definition = doc.block_definition_by_name("part").unwrap().id();
    let invalid = doc
        .add_block_instance(
            BlockReference::try_new(
                definition,
                AffineTransform3::try_nonuniform_scale(p(0., 0., 0.), [2., 3., 4.]).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let before = format!("{doc:?}");
    assert!(doc.open_block_edit(invalid).is_err());
    assert_eq!(format!("{doc:?}"), before);
    doc.open_block_edit(root).unwrap();
    let before = format!("{doc:?}");
    assert!(doc.open_block_edit(root).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn protected_model_records_stay_uneditable_even_after_unlock() {
    let (mut doc, peer, root) = fixture();
    doc.open_block_edit(root).unwrap();
    doc.set_objects_locked([peer], false).unwrap();
    assert!(!doc.is_object_selectable(peer));
    assert!(
        doc.select_objects_direct([peer], SelectionMode::Replace)
            .is_err()
    );
    assert!(
        doc.transform_objects(
            [peer],
            AffineTransform3::from_translation(Vector3::try_new(1., 0., 0.).unwrap())
        )
        .is_err()
    );
    let before = format!("{doc:?}");
    assert!(doc.delete_object(peer).is_err());
    assert!(doc.delete_objects([peer]).is_err());
    assert_eq!(format!("{doc:?}"), before);
    assert_eq!(doc.clear_objects(), 1);
    assert!(doc.object(peer).is_some());
    assert!(doc.object(root).is_some());
    doc.undo().unwrap();
    assert_eq!(doc.block_edit_objects().len(), 1);
    doc.discard_block_edit().unwrap();
    assert!(doc.is_object_selectable(peer));
}

#[test]
fn created_members_and_layers_replay_as_one_saved_model_change() {
    let (mut doc, _, root) = fixture();
    let original = doc.objects().cloned().collect::<Vec<_>>();
    doc.open_block_edit(root).unwrap();
    let layer = doc.add_layer("Edited", ColorRgb::new(21, 43, 65)).unwrap();
    doc.set_current_layer(layer).unwrap();
    doc.add_geometry(Geometry::Point(p(7., 8., 9.))).unwrap();
    assert_eq!(doc.save_block_edit().unwrap(), 2);
    doc.undo().unwrap();
    assert!(doc.layer(layer).is_none());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
    doc.redo().unwrap();
    assert!(doc.layer(layer).is_some());
    let Geometry::BlockInstance(i) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(i.members().len(), 2);
}

#[test]
fn cyclic_save_keeps_the_editor_and_model_history_for_correction_or_cancel() {
    let (mut doc, _, root) = fixture();
    let definition = doc.block_definition_by_name("part").unwrap().id();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.open_block_edit(root).unwrap();
    let recursive = doc
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    let edited = doc.objects().cloned().collect::<Vec<_>>();
    assert!(doc.save_block_edit().is_err());
    assert!(doc.is_block_editing());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), edited);
    doc.delete_objects([recursive]).unwrap();
    doc.save_block_edit().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn grouped_workspace_selection_and_membership_edits_preserve_model_groups() {
    let (mut doc, peer, root) = fixture();
    let group = doc.add_group(Some("Roots".into()), [peer, root]).unwrap();
    let members = doc.open_block_edit(root).unwrap();
    let member = members[0];
    doc.set_object_group_memberships(member, [group]).unwrap();
    doc.select_objects([member], SelectionMode::Replace)
        .unwrap();
    assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), [member]);
    let before = format!("{doc:?}");
    assert!(doc.select_command_results([peer]).is_err());
    assert!(doc.clear_object_group_memberships([member, peer]).is_err());
    assert!(doc.set_object_group_memberships(peer, []).is_err());
    assert!(doc.remove_group(group).is_err());
    assert_eq!(format!("{doc:?}"), before);
    doc.save_block_edit().unwrap();
    assert_eq!(doc.object(peer).unwrap().group_ids(), &[group]);
    assert_eq!(doc.object(root).unwrap().group_ids(), &[group]);
    assert_eq!(
        doc.group(group).unwrap().members().collect::<BTreeSet<_>>(),
        BTreeSet::from([peer, root])
    );
}

#[test]
fn added_external_objects_remain_in_the_model_and_join_the_definition_on_save() {
    let (mut doc, _, root) = fixture();
    let external = doc.add_geometry(Geometry::Point(p(14., 25., 36.))).unwrap();
    let group = doc.add_group(Some("Source".into()), [external]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.open_block_edit(root).unwrap();
    let added = doc.add_objects_to_block_edit([external, external]).unwrap();
    assert_eq!(added.len(), 1);
    assert!(doc.block_edit_objects().contains(&added[0]));
    doc.undo().unwrap();
    assert!(doc.object(added[0]).is_none());
    doc.redo().unwrap();
    assert_eq!(doc.save_block_edit().unwrap(), 2);
    assert_eq!(
        doc.object(external).unwrap(),
        before.iter().find(|o| o.id() == external).unwrap()
    );
    assert_eq!(doc.object(external).unwrap().group_ids(), &[group]);
    let Geometry::BlockInstance(i) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert!(matches!(&*i.members()[0].geometry,Geometry::Point(point) if *point==p(14.,25.,36.)));
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), before.len());
}

#[test]
fn removing_a_member_releases_its_world_geometry_and_replays_workspace_and_model_history() {
    let (mut doc, _, root) = fixture();
    let extra = doc.add_geometry(Geometry::Point(p(99., 0., 0.))).unwrap();
    doc.add_objects_to_block(root, [extra]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let members = doc.open_block_edit(root).unwrap();
    let released = members[0];
    let world = doc.object(released).unwrap().clone();
    doc.select_objects([released], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        doc.remove_objects_from_block_edit([released, released])
            .unwrap(),
        1
    );
    assert!(!doc.is_object_selectable(released));
    assert!(doc.delete_object(released).is_err());
    doc.undo().unwrap();
    assert!(doc.block_edit_objects().contains(&released));
    assert!(doc.is_selected(released));
    doc.redo().unwrap();
    assert_eq!(doc.save_block_edit().unwrap(), 1);
    assert_eq!(doc.object(released).unwrap(), &world);
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
}

#[test]
fn base_point_changes_shift_shared_geometry_while_released_objects_keep_their_world_positions() {
    let (mut doc, peer, root) = fixture();
    let extra = doc.add_geometry(Geometry::Point(p(99., 0., 0.))).unwrap();
    doc.add_objects_to_block(root, [extra]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let members = doc.open_block_edit(root).unwrap();
    let released = members[1];
    let world = doc.object(released).unwrap().clone();
    doc.remove_objects_from_block_edit([released]).unwrap();
    assert_eq!(doc.block_edit_base_point(), Some(p(10., 20., 30.)));
    assert!(doc.set_block_edit_base_point(p(12., 17., 34.)).unwrap());
    doc.undo().unwrap();
    assert_eq!(doc.block_edit_base_point(), Some(p(10., 20., 30.)));
    doc.redo().unwrap();
    doc.save_block_edit().unwrap();
    assert_eq!(doc.object(released).unwrap(), &world);
    for (id, expected) in [(peer, p(-1., 5., -1.)), (root, p(9., 25., 29.))] {
        let Geometry::BlockInstance(i) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(matches!(&*i.members()[0].geometry,Geometry::Point(point) if *point==expected));
    }
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn member_controls_validate_whole_batches_and_discard_restores_the_exact_document() {
    let (mut doc, peer, root) = fixture();
    let external = doc.add_geometry(Geometry::Point(p(3., 4., 5.))).unwrap();
    doc.add_geometry(Geometry::Point(p(6., 7., 8.))).unwrap();
    doc.undo().unwrap();
    let original = format!("{doc:?}");
    let members = doc.open_block_edit(root).unwrap();
    let working = format!("{doc:?}");
    assert!(
        doc.add_objects_to_block_edit([external, members[0]])
            .is_err()
    );
    assert!(
        doc.remove_objects_from_block_edit([members[0], peer])
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), working);
    doc.add_objects_to_block_edit([external]).unwrap();
    doc.remove_objects_from_block_edit([members[0]]).unwrap();
    doc.set_block_edit_base_point(p(11., 22., 33.)).unwrap();
    doc.discard_block_edit().unwrap();
    assert_eq!(format!("{doc:?}"), original);
    assert!(doc.can_redo());
}

#[test]
fn saved_members_follow_temporary_object_chronology_and_undo_restores_the_catalog() {
    let (mut doc, peer, root) = fixture();
    let extra = doc.add_geometry(Geometry::Point(p(99., 0., 0.))).unwrap();
    doc.add_objects_to_block(root, [extra]).unwrap();
    let before = doc.block_definitions().cloned().collect::<Vec<_>>();
    doc.open_block_edit(root).unwrap();
    doc.save_block_edit().unwrap();
    let Geometry::BlockInstance(i) = doc.object(peer).unwrap().geometry() else {
        panic!()
    };
    assert!(matches!(&*i.members()[0].geometry,Geometry::Point(point) if *point==p(89.,-20.,-30.)));
    doc.undo().unwrap();
    assert_eq!(doc.block_definitions().cloned().collect::<Vec<_>>(), before);
    doc.open_block_edit(root).unwrap();
    for id in doc.block_edit_objects().into_iter().rev() {
        doc.transform_objects(
            [id],
            AffineTransform3::from_translation(Vector3::try_new(2., 0., 0.).unwrap()),
        )
        .unwrap();
    }
    doc.save_block_edit().unwrap();
    let Geometry::BlockInstance(i) = doc.object(peer).unwrap().geometry() else {
        panic!()
    };
    assert!(matches!(&*i.members()[0].geometry,Geometry::Point(point) if *point==p(3.,2.,3.)));
}

#[test]
fn copied_nested_references_clear_geometry_text_without_changing_the_original() {
    let (mut doc, _, root) = fixture();
    let point = doc.add_geometry(Geometry::Point(p(4., 5., 6.))).unwrap();
    let (_, child) = doc
        .create_block_from_objects("child", p(0., 0., 0.), [point])
        .unwrap();
    doc.set_object_geometry_user_text([child], "Shape", Some("nested"))
        .unwrap();
    doc.open_block_edit(root).unwrap();
    let copies = doc.add_objects_to_block_edit([child]).unwrap();
    assert!(
        doc.object(copies[0])
            .unwrap()
            .geometry_user_text()
            .is_empty()
    );
    doc.save_block_edit().unwrap();
    assert_eq!(
        doc.object(child)
            .unwrap()
            .geometry_user_text()
            .get("Shape")
            .map(String::as_str),
        Some("nested")
    );
    let definition = doc.block_definition_by_name("part").unwrap();
    assert!(matches!(
        definition.members()[0].content(),
        BlockContent::Reference(_)
    ));
    assert!(definition.members()[0].geometry_user_text().is_empty());
}
