use super::*;
use viboceros_geometry::{Point3, Vector3};
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn reference(id: BlockDefinitionId, x: f64) -> BlockReference {
    BlockReference::try_new(
        id,
        AffineTransform3::from_translation(Vector3::try_new(x, 0., 0.).unwrap()),
    )
    .unwrap()
}

#[test]
fn one_level_expansion_keeps_child_instances_and_replays_geometry_storage() {
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p(2., 3., 4.))).unwrap();
    let (child, object) = doc
        .create_block_from_objects("child", p(1., 0., 0.), [point])
        .unwrap();
    let (_, root) = doc
        .create_block_from_objects("parent", p(10., 0., 0.), [object])
        .unwrap();
    let original = doc.object(root).unwrap().geometry_snapshot().clone();
    doc.select_object(root, SelectionMode::Replace).unwrap();
    let plan = doc.prepare_block_explosion(root, false, 1).unwrap();
    assert_eq!(plan.output_count(), 1);
    let outputs = doc.commit_block_explosions(vec![plan], false).unwrap();
    assert!(doc.object(root).is_none());
    let Geometry::BlockInstance(instance) = doc.object(outputs[0]).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(instance.reference().definition(), child);
    assert_eq!(
        *instance.members()[0].geometry,
        Geometry::Point(p(2., 3., 4.))
    );
    doc.undo().unwrap();
    assert!(
        doc.object(root)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
    assert!(doc.object(outputs[0]).is_none());
    doc.redo().unwrap();
    assert!(doc.object(outputs[0]).is_some());
    assert_eq!(doc.block_definitions().len(), 2);
}

#[test]
fn member_attributes_text_and_scoped_groups_survive_recursive_expansion() {
    let mut doc = Document::default();
    let group = doc.add_empty_group(Some("prototype".into())).unwrap();
    let attrs = ObjectAttributes::on_layer(doc.current_layer_id())
        .with_name("leaf")
        .with_color_source(ObjectColorSource::Parent)
        .try_with_user_text("kind", "part")
        .unwrap();
    let member = BlockMember::new(
        BlockContent::Geometry(Geometry::Point(p(1., 0., 0.)).into()),
        attrs,
    )
    .try_with_group_ids(vec![group])
    .unwrap()
    .try_with_geometry_user_text("source", "original")
    .unwrap();
    let child = doc.add_block_definition("child", vec![member]).unwrap();
    let nested = |x| {
        BlockMember::new(
            BlockContent::Reference(reference(child, x)),
            ObjectAttributes::on_layer(doc.current_layer_id())
                .with_color_source(ObjectColorSource::Parent),
        )
    };
    let parent = doc
        .add_block_definition("parent", vec![nested(10.), nested(20.)])
        .unwrap();
    let root = doc
        .add_block_instance_with_attributes(
            reference(parent, 100.),
            ObjectAttributes::on_layer(doc.current_layer_id())
                .with_object_color(ColorRgb::new(30, 60, 90)),
        )
        .unwrap();
    let root_group = doc.add_group(Some("root group".into()), [root]).unwrap();
    doc.select_object(root, SelectionMode::Replace).unwrap();
    let plan = doc.prepare_block_explosion(root, true, 2).unwrap();
    let outputs = doc.commit_block_explosions(vec![plan], false).unwrap();
    let mut groups = Vec::new();
    for (id, x) in outputs.iter().zip([111., 121.]) {
        let object = doc.object(*id).unwrap();
        assert_eq!(object.geometry(), &Geometry::Point(p(x, 0., 0.)));
        assert_eq!(object.attributes().name(), Some("leaf"));
        assert_eq!(object.attributes().user_text()["kind"], "part");
        assert_eq!(object.geometry_user_text()["source"], "original");
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(30, 60, 90)
        );
        assert_eq!(
            object.attributes().color_source(),
            ObjectColorSource::Object
        );
        assert_eq!(object.top_group(), Some(root_group));
        groups.push(object.group_ids()[0]);
    }
    assert_ne!(groups[0], groups[1]);
    assert!(!groups.contains(&group));
    assert_eq!(doc.group(group).unwrap().members().len(), 0);
    doc.undo().unwrap();
    assert!(doc.object(root).is_some());
    for group in groups {
        assert!(doc.group(group).is_none());
    }
}

#[test]
fn stale_geometry_attributes_layers_and_tolerance_fail_without_mutating_redo() {
    let mut doc = Document::default();
    let layer = doc.add_layer("members", ColorRgb::BLACK).unwrap();
    let member = BlockMember::new(
        BlockContent::Geometry(Geometry::Point(p(1., 0., 0.)).into()),
        ObjectAttributes::on_layer(layer),
    );
    let definition = doc.add_block_definition("part", vec![member]).unwrap();
    let root = doc.add_block_instance(reference(definition, 10.)).unwrap();
    doc.select_object(root, SelectionMode::Replace).unwrap();
    assert!(doc.prepare_block_explosion(root, false, 0).is_err());
    let plan = doc.prepare_block_explosion(root, false, 1).unwrap();
    doc.set_layer_color(layer, ColorRgb::new(1, 2, 3)).unwrap();
    let before = format!("{doc:?}");
    assert_eq!(
        doc.commit_block_explosions(vec![plan], false),
        Err(DocumentError::StaleBlockExplosion)
    );
    assert_eq!(format!("{doc:?}"), before);
    let plan = doc.prepare_block_explosion(root, false, 1).unwrap();
    doc.set_tolerance(Tolerance::try_new(0.01, 1e-8, 1e-8).unwrap());
    let before = format!("{doc:?}");
    assert!(doc.commit_block_explosions(vec![plan], false).is_err());
    assert_eq!(format!("{doc:?}"), before);
    let plan = doc.prepare_block_explosion(root, false, 1).unwrap();
    doc.transform_objects(
        [root],
        AffineTransform3::from_translation(Vector3::try_new(1., 0., 0.).unwrap()),
    )
    .unwrap();
    let before = format!("{doc:?}");
    assert!(doc.commit_block_explosions(vec![plan], false).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn locked_member_layer_and_raw_hidden_state_are_retained_without_partial_insertion() {
    let mut doc = Document::default();
    let layer = doc.add_layer("member", ColorRgb::BLACK).unwrap();
    let attrs = ObjectAttributes::on_layer(layer).with_file_state(false, true);
    let definition = doc
        .add_block_definition(
            "part",
            vec![BlockMember::new(
                BlockContent::Geometry(Geometry::Point(p(1., 0., 0.)).into()),
                attrs,
            )],
        )
        .unwrap();
    let root = doc.add_block_instance(reference(definition, 0.)).unwrap();
    doc.select_object(root, SelectionMode::Replace).unwrap();
    doc.set_layer_locked(layer, true).unwrap();
    let plan = doc.prepare_block_explosion(root, false, 1).unwrap();
    let output = doc.commit_block_explosions(vec![plan], false).unwrap()[0];
    let attrs = doc.object(output).unwrap().attributes();
    assert_eq!(attrs.layer_id(), layer);
    assert!(!attrs.is_visible());
    assert!(attrs.is_locked());
    assert!(doc.is_selected(output));
    assert!(!doc.is_object_selectable(output));
}

#[test]
fn group_output_and_outer_transaction_rollback_restore_original_memberships() {
    let mut doc = Document::default();
    let definition = doc
        .add_block_definition(
            "part",
            vec![BlockMember::new(
                BlockContent::Geometry(Geometry::Point(p(1., 0., 0.)).into()),
                ObjectAttributes::on_layer(doc.current_layer_id()),
            )],
        )
        .unwrap();
    let root = doc.add_block_instance(reference(definition, 10.)).unwrap();
    doc.select_object(root, SelectionMode::Replace).unwrap();
    let original = doc.object(root).unwrap().clone();
    let groups = doc.groups().len();
    doc.begin_transaction("Explode").unwrap();
    let plan = doc.prepare_block_explosion(root, true, 1).unwrap();
    let output = doc.commit_block_explosions(vec![plan], true).unwrap()[0];
    assert!(doc.object(output).unwrap().top_group().is_some());
    doc.rollback_transaction().unwrap();
    assert_eq!(doc.object(root).unwrap(), &original);
    assert_eq!(doc.groups().len(), groups);
}
