use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn t(x: f64, y: f64, z: f64) -> AffineTransform3 {
    AffineTransform3::from_translation(Vector3::try_new(x, y, z).unwrap())
}
fn fixture() -> (Document, ObjectId, ObjectId, BlockDefinitionId) {
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p(1., 2., 3.))).unwrap();
    let (child, child_root) = doc
        .create_block_from_objects("child", p(0., 0., 0.), [point])
        .unwrap();
    doc.transform_objects([child_root], t(7., 8., 9.)).unwrap();
    let point = doc.add_geometry(Geometry::Point(p(4., 5., 6.))).unwrap();
    let (parent, _) = doc
        .create_block_from_objects("parent", p(0., 0., 0.), [child_root, point])
        .unwrap();
    let root = doc
        .add_block_instance(BlockReference::try_new(parent, t(10., 20., 30.)).unwrap())
        .unwrap();
    let peer = doc
        .add_block_instance(BlockReference::try_new(child, t(40., 50., 60.)).unwrap())
        .unwrap();
    (doc, root, peer, child)
}
fn child_path(doc: &Document, child: BlockDefinitionId) -> Vec<usize> {
    doc.block_edit_tree()
        .unwrap()
        .into_iter()
        .find(|n| n.definition == child)
        .unwrap()
        .path
}
#[test]
fn child_context_uses_composed_placement_and_updates_every_shared_use() {
    let (mut doc, root, peer, child) = fixture();
    let parent = doc.object(root).unwrap().geometry().clone();
    doc.open_block_edit(root).unwrap();
    let path = child_path(&doc, child);
    let members = doc.switch_block_edit_context(&path).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(doc.block_edit_definition(), Some(child));
    assert!(
        matches!(doc.object(members[0]).unwrap().geometry(),Geometry::Point(point) if *point==p(18.,30.,42.))
    );
    for object in doc.objects().filter(|o| !members.contains(&o.id())) {
        assert!(!doc.is_object_selectable(object.id()));
    }
    doc.transform_objects(members, t(2., -3., 4.)).unwrap();
    doc.save_block_edit().unwrap();
    let Geometry::BlockInstance(instance) = doc.object(peer).unwrap().geometry() else {
        panic!()
    };
    assert!(
        matches!(&*instance.members()[0].geometry,Geometry::Point(point) if *point==p(43.,49.,67.))
    );
    let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert!(
        instance
            .members()
            .iter()
            .any(|m| matches!(&*m.geometry,Geometry::Point(point) if *point==p(20.,27.,46.)))
    );
    doc.undo().unwrap();
    assert_eq!(doc.object(root).unwrap().geometry(), &parent);
}
#[test]
fn parent_edits_survive_child_visits_and_navigation_history_restores_the_exact_scene() {
    let (mut doc, root, _, child) = fixture();
    doc.open_block_edit(root).unwrap();
    let point = doc
        .block_edit_objects()
        .into_iter()
        .find(|id| matches!(doc.object(*id).unwrap().geometry(), Geometry::Point(_)))
        .unwrap();
    doc.transform_objects([point], t(5., 0., 0.)).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let path = child_path(&doc, child);
    doc.switch_block_edit_context(&path).unwrap();
    let after = doc.objects().cloned().collect::<Vec<_>>();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.block_edit_path(), Some([].as_slice()));
    doc.redo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
    doc.transform_objects(doc.block_edit_objects(), t(2., 0., 0.))
        .unwrap();
    doc.switch_block_edit_context(&[]).unwrap();
    assert!(doc.block_edit_objects().iter().any(|id|matches!(doc.object(*id).unwrap().geometry(),Geometry::Point(point) if *point==p(19.,25.,36.))));
    doc.save_block_edit().unwrap();
    assert_eq!(doc.undo_label(), Some("BlockEdit"));
}
#[test]
fn rejected_paths_preserve_state_and_nested_discard_can_undo_context_acceptance() {
    let (mut doc, root, _, child) = fixture();
    doc.add_geometry(Geometry::Point(p(99., 0., 0.))).unwrap();
    doc.undo().unwrap();
    let original = doc.clone();
    doc.open_block_edit(root).unwrap();
    let scene = format!("{doc:?}");
    assert!(doc.switch_block_edit_context(&[usize::MAX]).is_err());
    assert_eq!(format!("{doc:?}"), scene);
    let path = child_path(&doc, child);
    doc.switch_block_edit_context(&path).unwrap();
    doc.add_geometry(Geometry::Point(p(5., 6., 7.))).unwrap();
    doc.discard_block_edit().unwrap();
    assert!(!doc.is_block_editing());
    assert_eq!(doc.undo_label(), Some("BlockEdit"));
    doc.undo().unwrap();
    assert_eq!(
        doc.objects().cloned().collect::<Vec<_>>(),
        original.objects().cloned().collect::<Vec<_>>()
    );
    assert_eq!(
        doc.block_definitions().cloned().collect::<Vec<_>>(),
        original.block_definitions().cloned().collect::<Vec<_>>()
    );
    assert!(doc.can_redo());
}

#[test]
fn nonuniform_child_context_rejection_is_atomic_and_released_objects_survive_navigation() {
    let (mut doc, root, _, child) = fixture();
    let parent = doc.object(root).unwrap().geometry().clone();
    let Geometry::BlockInstance(instance) = parent else {
        panic!()
    };
    let parent_id = instance.reference().definition();
    let mut members = doc.block_definition(parent_id).unwrap().members().to_vec();
    for member in &mut members {
        if matches!(member.content(), BlockContent::Reference(_)) {
            *member = BlockMember::new(
                BlockContent::Reference(
                    BlockReference::try_new(
                        child,
                        AffineTransform3::try_nonuniform_scale(p(0., 0., 0.), [2., 3., 4.])
                            .unwrap(),
                    )
                    .unwrap(),
                ),
                member.attributes().clone(),
            );
        }
    }
    doc.replace_block_definition_members(parent_id, members)
        .unwrap();
    doc.open_block_edit(root).unwrap();
    let node = doc
        .block_edit_tree()
        .unwrap()
        .into_iter()
        .find(|n| n.definition == child)
        .unwrap();
    assert!(!node.editable);
    let before = format!("{doc:?}");
    assert!(doc.switch_block_edit_context(&node.path).is_err());
    assert_eq!(format!("{doc:?}"), before);
    doc.discard_block_edit().unwrap();
    let (mut doc, root, _, child) = fixture();
    doc.open_block_edit(root).unwrap();
    let released = doc
        .block_edit_objects()
        .into_iter()
        .find(|id| matches!(doc.object(*id).unwrap().geometry(), Geometry::Point(_)))
        .unwrap();
    let world = doc.object(released).unwrap().clone();
    doc.remove_objects_from_block_edit([released]).unwrap();
    let path = child_path(&doc, child);
    doc.switch_block_edit_context(&path).unwrap();
    doc.save_block_edit().unwrap();
    assert_eq!(doc.object(released).unwrap(), &world);
}

#[test]
fn grouped_context_background_never_leaks_temporary_ids_into_the_saved_model() {
    let (mut doc, root, _, child) = fixture();
    let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    let parent_id = instance.reference().definition();
    let group = doc.add_empty_group(Some("Prototype".into())).unwrap();
    let members = doc
        .block_definition(parent_id)
        .unwrap()
        .members()
        .iter()
        .cloned()
        .map(|m| m.try_with_group_ids(vec![group]).unwrap())
        .collect();
    doc.replace_block_definition_members(parent_id, members)
        .unwrap();
    doc.open_block_edit(root).unwrap();
    let path = child_path(&doc, child);
    doc.switch_block_edit_context(&path).unwrap();
    doc.save_block_edit().unwrap();
    let model = doc.objects().map(|o| o.id()).collect::<BTreeSet<_>>();
    for group in doc.groups() {
        assert!(group.members().all(|id| model.contains(&id)));
    }
}

#[test]
fn nested_discard_keeps_accepted_context_edits_and_discards_only_the_current_context() {
    let (mut doc, root, peer, child) = fixture();
    let original = doc.objects().cloned().collect::<Vec<_>>();
    doc.open_block_edit(root).unwrap();
    let parent_point = doc
        .block_edit_objects()
        .into_iter()
        .find(|id| matches!(doc.object(*id).unwrap().geometry(), Geometry::Point(_)))
        .unwrap();
    doc.transform_objects([parent_point], t(5., 0., 0.))
        .unwrap();
    let path = child_path(&doc, child);
    doc.switch_block_edit_context(&path).unwrap();
    doc.transform_objects(doc.block_edit_objects(), t(2., -3., 4.))
        .unwrap();
    doc.discard_block_edit().unwrap();
    let Geometry::BlockInstance(instance) = doc.object(peer).unwrap().geometry() else {
        panic!()
    };
    assert!(
        matches!(&*instance.members()[0].geometry,Geometry::Point(point) if *point==p(41.,52.,63.))
    );
    let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert!(
        instance
            .members()
            .iter()
            .any(|m| matches!(&*m.geometry,Geometry::Point(point) if *point==p(19.,25.,36.)))
    );
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
}
