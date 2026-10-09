use super::*;

#[test]
fn unique_rebind_preserves_root_identity_metadata_and_groups_but_drops_prototype_groups() {
    let mut doc = Document::default();
    let p = point(&mut doc, 1.);
    let q = point(&mut doc, 2.);
    doc.add_group(None, [p, q]).unwrap();
    let (original, a) = doc
        .create_block_from_objects(
            "part",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p, q],
        )
        .unwrap();
    let b = doc.add_block_instance(reference(original)).unwrap();
    let root_group = doc.add_group(None, [a, b]).unwrap();
    doc.set_object_geometry_user_text([b], "RootShape", Some("data"))
        .unwrap();
    let before = doc.object(b).unwrap().clone();
    let prototype = doc.block_definition(original).unwrap().members()[0].clone();
    let copy = doc.make_block_instances_unique("unique", [b]).unwrap();
    assert_ne!(copy, original);
    assert!(
        doc.block_definition(copy).unwrap().members()[0]
            .group_ids()
            .is_empty()
    );
    let BlockContent::Geometry(before_geo) = prototype.content() else {
        panic!()
    };
    let BlockContent::Geometry(after_geo) =
        doc.block_definition(copy).unwrap().members()[0].content()
    else {
        panic!()
    };
    assert!(after_geo.shares_storage_with(before_geo));
    let object = doc.object(b).unwrap();
    assert_eq!(object.attributes(), before.attributes());
    assert_eq!(object.geometry_user_text(), before.geometry_user_text());
    assert_eq!(object.group_ids(), [root_group]);
    assert!(
        matches!(doc.object(a).unwrap().geometry(),Geometry::BlockInstance(i) if i.reference().definition()==original)
    );
    doc.undo().unwrap();
    assert_eq!(doc.object(b).unwrap(), &before);
    assert!(doc.block_definition(copy).is_none());
    doc.redo().unwrap();
    assert!(
        matches!(doc.object(b).unwrap().geometry(),Geometry::BlockInstance(i) if i.reference().definition()==copy)
    );
}

#[test]
fn duplicate_definitions_share_nested_children_and_invalid_names_preserve_redo() {
    let mut doc = Document::default();
    let p = point(&mut doc, 1.);
    let (leaf, child) = doc
        .create_block_from_objects(
            "leaf",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p],
        )
        .unwrap();
    let (parent, _) = doc
        .create_block_from_objects(
            "parent",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [child],
        )
        .unwrap();
    let copy = doc.duplicate_block_definition(parent, "copy").unwrap();
    assert!(
        matches!(doc.block_definition(copy).unwrap().members()[0].content(),BlockContent::Reference(r)if r.definition()==leaf)
    );
    assert_eq!(
        doc.block_definition_info()
            .unwrap()
            .iter()
            .find(|r| r.id == copy)
            .unwrap()
            .total_instances(),
        0
    );
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    assert!(doc.duplicate_block_definition(parent, "leaf").is_err());
    assert_eq!(format!("{doc:?}"), before);
    doc.redo().unwrap();
    assert!(doc.block_definition(copy).is_some());
}

#[test]
fn unique_mixed_or_uneditable_roots_fail_before_catalog_and_history_changes() {
    let mut doc = Document::default();
    let p = point(&mut doc, 1.);
    let (_, a) = doc
        .create_block_from_objects(
            "a",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p],
        )
        .unwrap();
    let p = point(&mut doc, 2.);
    let (_, b) = doc
        .create_block_from_objects(
            "b",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p],
        )
        .unwrap();
    let before = format!("{doc:?}");
    assert!(doc.make_block_instances_unique("mixed", [a, b]).is_err());
    assert_eq!(format!("{doc:?}"), before);
    doc.set_objects_locked([a], true).unwrap();
    let before = format!("{doc:?}");
    assert!(doc.make_block_instances_unique("locked", [a]).is_err());
    assert_eq!(format!("{doc:?}"), before);
}
fn point(doc: &mut Document, x: f64) -> ObjectId {
    doc.add_geometry(Geometry::Point(
        viboceros_geometry::Point3::try_new(x, 0., 0.).unwrap(),
    ))
    .unwrap()
}
fn reference(id: BlockDefinitionId) -> BlockReference {
    BlockReference::try_new(id, viboceros_geometry::AffineTransform3::identity()).unwrap()
}

#[test]
fn use_counts_propagate_repeated_placements_and_distinguish_catalog_references() {
    let mut doc = Document::default();
    let p = point(&mut doc, 1.);
    let (leaf, a) = doc
        .create_block_from_objects(
            "leaf",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p],
        )
        .unwrap();
    let b = doc.add_block_instance(reference(leaf)).unwrap();
    let (parent, root) = doc
        .create_block_from_objects(
            "parent",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [a, b],
        )
        .unwrap();
    let other = doc.add_block_instance(reference(parent)).unwrap();
    let info = doc.block_definition_info().unwrap();
    let leaf_info = info.iter().find(|r| r.id == leaf).unwrap();
    assert_eq!(
        (
            leaf_info.top_level_instances,
            leaf_info.nested_instances,
            leaf_info.definition_references
        ),
        (0, 4, 2)
    );
    doc.delete_objects([root, other]).unwrap();
    let info = doc.block_definition_info().unwrap();
    let leaf_info = info.iter().find(|r| r.id == leaf).unwrap();
    assert_eq!(leaf_info.total_instances(), 0);
    assert_eq!(leaf_info.definition_references, 2);
    assert!(doc.delete_block_definition_and_instances(leaf).is_err());
}

#[test]
fn rename_preserves_identity_and_geometry_storage_and_invalid_names_preserve_redo() {
    let mut doc = Document::default();
    let p = point(&mut doc, 1.);
    let (id, root) = doc
        .create_block_from_objects(
            "part",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p],
        )
        .unwrap();
    let stored = doc.object(root).unwrap().geometry_snapshot().clone();
    doc.rename_block_definition(id, "Part A").unwrap();
    assert_eq!(doc.block_definition(id).unwrap().name(), "Part A");
    assert!(
        doc.object(root)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&stored)
    );
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    assert!(doc.rename_block_definition(id, "").is_err());
    assert_eq!(format!("{doc:?}"), before);
    assert!(!doc.rename_block_definition(id, "part").unwrap());
    assert_eq!(format!("{doc:?}"), before);
    doc.redo().unwrap();
    assert_eq!(doc.block_definition(id).unwrap().name(), "Part A");
}

#[test]
fn definition_deletion_removes_all_roots_and_replays_protected_states_and_groups() {
    let mut doc = Document::default();
    let p = point(&mut doc, 1.);
    let (id, a) = doc
        .create_block_from_objects(
            "part",
            viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
            [p],
        )
        .unwrap();
    let attrs = ObjectAttributes::on_layer(doc.current_layer_id()).with_file_state(false, true);
    let b = doc
        .add_block_instance_with_attributes(reference(id), attrs)
        .unwrap();
    let group = doc.add_group(None, [a, b]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert_eq!(doc.delete_block_definition_and_instances(id).unwrap(), 2);
    assert!(doc.block_definition(id).is_none());
    assert_eq!(doc.objects().len(), 0);
    assert_eq!(doc.group(group).unwrap().members().len(), 0);
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.group(group).unwrap().members().len(), 2);
    doc.redo().unwrap();
    assert_eq!(doc.objects().len(), 0);
}
