use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn capture_retains_raw_geometry_text_from_piece_copy_inputs() {
    let mut doc = Document::default();
    let source = doc.add_geometry(Geometry::Point(p(1., 0., 0.))).unwrap();
    let text = BTreeMap::from([
        ("Tag".to_owned(), "one".to_owned()),
        ("tag".to_owned(), "two".to_owned()),
        ("empty".to_owned(), String::new()),
    ]);
    let copied = doc
        .copy_object_pieces_with_metadata_into_source_groups([(
            source,
            Geometry::Point(p(2., 0., 0.)),
            text.clone(),
        )])
        .unwrap()[0];
    let (definition, root) = doc
        .create_block_from_objects("raw", p(0., 0., 0.), [copied])
        .unwrap();
    assert_eq!(
        doc.block_definition(definition).unwrap().members()[0].geometry_user_text(),
        &text
    );
    let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(instance.members()[0].geometry_user_text, text);
}

#[test]
fn creation_clones_top_groups_and_keeps_unselected_peers_outside_the_definition() {
    let mut doc = Document::default();
    let ids = (0..4)
        .map(|i| {
            doc.add_geometry(Geometry::Point(p(f64::from(i), 0., 0.)))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let lower = doc.add_group(None, [ids[0], ids[1], ids[3]]).unwrap();
    let upper = doc.add_group(None, [ids[1], ids[2]]).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let (definition, root) = doc
        .create_block_from_objects("groups", p(0., 0., 0.), ids[..3].iter().copied())
        .unwrap();
    let members = doc.block_definition(definition).unwrap().members();
    let a = members[0].group_ids()[0];
    let b = members[1].group_ids()[0];
    assert_ne!(a, b);
    assert!(![lower, upper].contains(&a));
    assert!(![lower, upper].contains(&b));
    assert_eq!(members[1].group_ids(), [b]);
    assert_eq!(members[2].group_ids(), [b]);
    assert_eq!(doc.object(ids[3]).unwrap().group_ids(), [lower]);
    assert_eq!(doc.group(a).unwrap().members().len(), 0);
    assert_eq!(doc.group(b).unwrap().members().len(), 0);
    assert!(doc.object(root).unwrap().group_ids().is_empty());
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.group(a).is_none());
    assert!(doc.group(b).is_none());
    doc.redo().unwrap();
    assert_eq!(
        doc.block_definition(definition).unwrap().members()[0].group_ids(),
        [a]
    );
}

#[test]
fn grouped_source_cycle_failure_preserves_groups_objects_and_redo() {
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p(1., 0., 0.))).unwrap();
    let (_, root) = doc
        .create_block_from_objects("part", p(0., 0., 0.), [point])
        .unwrap();
    doc.add_group(None, [root]).unwrap();
    doc.add_geometry(Geometry::Point(p(2., 0., 0.))).unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    assert!(
        doc.create_block_from_objects("part", p(0., 0., 0.), [root])
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}

#[test]
fn creation_normalizes_and_replays_source_metadata_groups_and_storage() {
    let mut doc = Document::default();
    let a = doc.add_geometry(Geometry::Point(p(11., 22., 33.))).unwrap();
    let b = doc.add_geometry(Geometry::Point(p(12., 23., 34.))).unwrap();
    doc.set_object_geometry_user_text([a], "code", Some("point data"))
        .unwrap();
    doc.set_object_user_text([a], "material", Some("steel"))
        .unwrap();
    let group = doc.add_group(Some("members".into()), [a, b]).unwrap();
    doc.select_objects([a, b], SelectionMode::Replace).unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let (definition, object) = doc
        .create_block_from_objects("Part A", p(10., 20., 30.), [a, b])
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.undo_label(), Some("Block"));
    let stored = doc.block_definition(definition).unwrap();
    let BlockContent::Geometry(geometry) = stored.members()[0].content() else {
        panic!()
    };
    assert_eq!(**geometry, Geometry::Point(p(1., 2., 3.)));
    let copied_group = stored.members()[0].group_ids()[0];
    assert_ne!(copied_group, group);
    assert_eq!(stored.members()[1].group_ids(), [copied_group]);
    assert_eq!(
        stored.members()[0].geometry_user_text()["code"],
        "point data"
    );
    assert_eq!(
        stored.members()[0].attributes().user_text()["material"],
        "steel"
    );
    assert_eq!(
        doc.remove_group(copied_group),
        Err(DocumentError::GroupUsedByBlocks(copied_group))
    );
    let Geometry::BlockInstance(instance) = doc.object(object).unwrap().geometry() else {
        panic!()
    };
    for (placed, source) in instance.members().iter().zip(&before) {
        assert_eq!(&*placed.geometry, source.geometry());
    }
    assert!(doc.object(object).unwrap().group_ids().is_empty());
    assert_eq!(doc.selected_object_count(), 0);
    let snapshot = doc.object(object).unwrap().geometry_snapshot().clone();
    doc.undo().unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(doc.block_definitions().len(), 0);
    assert!(doc.group(copied_group).is_none());
    doc.redo().unwrap();
    assert!(
        doc.object(object)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&snapshot)
    );
}

#[test]
fn nested_capture_and_same_name_redefinition_keep_definition_relationships() {
    let mut doc = Document::default();
    let source = doc.add_geometry(Geometry::Point(p(2., 0., 0.))).unwrap();
    let (child, child_object) = doc
        .create_block_from_objects("child", p(1., 0., 0.), [source])
        .unwrap();
    let (_, parent_object) = doc
        .create_block_from_objects("parent", p(10., 0., 0.), [child_object])
        .unwrap();
    let Geometry::BlockInstance(parent) = doc.object(parent_object).unwrap().geometry() else {
        panic!()
    };
    let definition = parent.reference().definition();
    let BlockContent::Reference(reference) =
        doc.block_definition(definition).unwrap().members()[0].content()
    else {
        panic!()
    };
    assert_eq!(reference.definition(), child);
    let other = doc
        .add_block_instance(
            BlockReference::try_new(
                child,
                AffineTransform3::from_translation(Vector3::try_new(20., 0., 0.).unwrap()),
            )
            .unwrap(),
        )
        .unwrap();
    let replacement = doc.add_geometry(Geometry::Point(p(7., 0., 0.))).unwrap();
    let (reused, _) = doc
        .create_block_from_objects("CHILD", p(3., 0., 0.), [replacement])
        .unwrap();
    assert_eq!(reused, child);
    for (object, expected) in [(other, 24.), (parent_object, 5.)] {
        let Geometry::BlockInstance(instance) = doc.object(object).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(
            *instance.members()[0].geometry,
            Geometry::Point(p(expected, 0., 0.))
        );
    }
}

#[test]
fn cycle_overflow_missing_sources_and_groups_fail_without_consuming_redo() {
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p(1., 0., 0.))).unwrap();
    let (definition, object) = doc
        .create_block_from_objects("part", p(0., 0., 0.), [point])
        .unwrap();
    doc.add_geometry(Geometry::Point(p(3., 0., 0.))).unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    assert!(
        doc.create_block_from_objects("part", p(0., 0., 0.), [object])
            .is_err()
    );
    assert!(
        doc.create_block_from_objects("invalid", p(0., 0., 0.), [ObjectId::new()])
            .is_err()
    );
    assert!(
        doc.create_block_from_objects("empty", p(0., 0., 0.), [])
            .is_err()
    );
    let member = BlockMember::new(
        BlockContent::Reference(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        ),
        ObjectAttributes::on_layer(doc.current_layer_id()),
    )
    .try_with_group_ids(vec![GroupId::new()])
    .unwrap();
    assert!(doc.add_block_definition("bad group", vec![member]).is_err());
    assert_eq!(format!("{doc:?}"), before);
    let huge = doc
        .add_geometry(Geometry::Point(p(f64::MAX, 0., 0.)))
        .unwrap();
    let before = format!("{doc:?}");
    assert!(
        doc.create_block_from_objects("overflow", p(-f64::MAX, 0., 0.), [huge])
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn enclosing_transaction_rollback_restores_sources_and_removes_the_definition() {
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p(1., 0., 0.))).unwrap();
    let original = doc.object(point).unwrap().clone();
    doc.begin_transaction("assembly").unwrap();
    let (_, root) = doc
        .create_block_from_objects("part", p(0., 0., 0.), [point])
        .unwrap();
    let Geometry::BlockInstance(instance) = doc.object(root).unwrap().geometry() else {
        panic!()
    };
    assert!(
        instance.members()[0]
            .geometry
            .shares_storage_with(original.geometry_snapshot())
    );
    doc.rollback_transaction().unwrap();
    assert_eq!(doc.object(point).unwrap(), &original);
    assert_eq!(doc.block_definitions().len(), 0);
}
