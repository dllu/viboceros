use super::*;
use viboceros_geometry::{Point3, Vector3};

#[test]
fn reflected_brep_instances_keep_volume_and_certified_boundaries_across_unit_rescaling() {
    use viboceros_geometry::{Brep, Frame3};
    let mut document = Document::default();
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        document.tolerance(),
    )
    .unwrap();
    let brep = Brep::try_box(frame, [[0., 1.]; 3], document.tolerance()).unwrap();
    let definition = document
        .add_block_definition(
            "box",
            vec![BlockMember::new(
                BlockContent::Geometry(Geometry::Brep(brep).into()),
                ObjectAttributes::on_layer(document.current_layer_id()),
            )],
        )
        .unwrap();
    let transform = AffineTransform3::try_new(
        [[-2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(1000., 2000., 3000.).unwrap(),
    )
    .unwrap();
    let object = document
        .add_block_instance(BlockReference::try_new(definition, transform).unwrap())
        .unwrap();
    let original = document.object(object).unwrap().geometry_snapshot().clone();
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    let Geometry::Brep(brep) = &*instance(&document, object).members()[0].geometry else {
        panic!()
    };
    assert!((brep.signed_volume(Tolerance::NUMERICAL_VALIDATION).unwrap() - 24e-9).abs() < 1e-18);
    assert_eq!(brep.bounds().min().to_array(), [0.998, 2., 3.]);
    assert_eq!(brep.bounds().max().to_array(), [1., 2.003, 3.004]);
    for face in brep.faces() {
        for trim in face.loops().iter().flat_map(|boundary| boundary.trims()) {
            if let Some(edge) = trim.edge() {
                let curve = brep.edges()[edge].curve();
                let curve = if trim.is_reversed_3d() {
                    curve.reversed().unwrap()
                } else {
                    curve.clone()
                };
                assert!(
                    face.surface()
                        .parameter_curve_deviation_bound(trim.curve(), &curve, 1e-9)
                        .unwrap()
                        .is_some()
                );
            }
        }
    }
    document.undo().unwrap();
    assert!(
        document
            .object(object)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
    let Geometry::Brep(brep) = &*instance(&document, object).members()[0].geometry else {
        panic!()
    };
    assert!((brep.signed_volume(Tolerance::NUMERICAL_VALIDATION).unwrap() - 24.).abs() < 1e-10);
}

fn point(x: f64) -> Geometry {
    Geometry::Point(Point3::try_new(x, 0., 0.).unwrap())
}
fn leaf(document: &Document, x: f64) -> BlockMember {
    BlockMember::new(
        BlockContent::Geometry(point(x).into()),
        ObjectAttributes::on_layer(document.current_layer_id()),
    )
}
fn placement(id: BlockDefinitionId, x: f64) -> BlockReference {
    BlockReference::try_new(
        id,
        AffineTransform3::from_translation(Vector3::try_new(x, 0., 0.).unwrap()),
    )
    .unwrap()
}
fn instance(document: &Document, id: ObjectId) -> &BlockInstance {
    let Geometry::BlockInstance(instance) = document.object(id).unwrap().geometry() else {
        panic!("expected instance")
    };
    instance
}
fn x(document: &Document, id: ObjectId) -> f64 {
    let Geometry::Point(point) = &*instance(document, id).members()[0].geometry else {
        panic!()
    };
    point.x()
}

#[test]
fn insertion_selection_groups_metadata_and_deletion_use_one_instance_object() {
    let mut document = Document::default();
    let definition = document
        .add_block_definition("part", vec![leaf(&document, 2.)])
        .unwrap();
    let reference = placement(definition, 10.);
    let object = document
        .add_block_instance_with_attributes(
            reference,
            ObjectAttributes::on_layer(document.current_layer_id()).with_name("placed"),
        )
        .unwrap();
    document
        .set_object_geometry_user_text([object], "code", Some("part 7"))
        .unwrap();
    document
        .set_object_user_text([object], "owner", Some("fixture"))
        .unwrap();
    let group = document
        .add_group(Some("assembly".into()), [object])
        .unwrap();
    document
        .select_object(object, SelectionMode::Replace)
        .unwrap();
    assert_eq!(document.objects().len(), 1);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [object]);
    assert_eq!(instance(&document, object).reference(), reference);
    assert_eq!(x(&document, object), 12.);
    assert_eq!(document.object(object).unwrap().group_ids(), [group]);
    assert_eq!(
        document.object(object).unwrap().attributes().name(),
        Some("placed")
    );
    assert_eq!(document.enable_control_points([object]).unwrap(), 0);
    assert!(document.is_selected(object));
    let snapshot = document.object(object).unwrap().geometry_snapshot().clone();
    document.delete_object(object).unwrap();
    assert_eq!(document.block_definitions().len(), 1);
    document.undo().unwrap();
    let restored = document.object(object).unwrap();
    assert!(restored.geometry_snapshot().shares_storage_with(&snapshot));
    assert_eq!(restored.geometry_user_text()["code"], "part 7");
    assert_eq!(restored.attributes().user_text()["owner"], "fixture");
    document.redo().unwrap();
    document.remove_block_definition(definition).unwrap();
}

#[test]
fn definition_edits_refresh_only_changed_instances_and_replay_immutable_cache_snapshots() {
    let mut document = Document::default();
    let part = document
        .add_block_definition("part", vec![leaf(&document, 1.)])
        .unwrap();
    let independent = document
        .add_block_definition("independent", vec![leaf(&document, 3.)])
        .unwrap();
    let a = document.add_block_instance(placement(part, 10.)).unwrap();
    let b = document.add_block_instance(placement(part, 20.)).unwrap();
    let c = document
        .add_block_instance(placement(independent, 30.))
        .unwrap();
    let before_a = document.object(a).unwrap().geometry_snapshot().clone();
    let before_c = document.object(c).unwrap().geometry_snapshot().clone();
    document.select_object(a, SelectionMode::Replace).unwrap();
    document
        .replace_block_definition_members(part, vec![leaf(&document, 2.)])
        .unwrap();
    assert_eq!(
        [x(&document, a), x(&document, b), x(&document, c)],
        [12., 22., 33.]
    );
    assert!(
        document
            .object(c)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&before_c)
    );
    assert!(document.is_selected(a));
    assert_eq!(document.last_changed_objects, BTreeSet::from([a, b]));
    let after = document.object(a).unwrap().geometry_snapshot().clone();
    document.undo().unwrap();
    assert!(
        document
            .object(a)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&before_a)
    );
    let before_noop = format!("{document:?}");
    assert!(
        !document
            .replace_block_definition_members(part, vec![leaf(&document, 1.)])
            .unwrap()
    );
    assert_eq!(format!("{document:?}"), before_noop);
    document.redo().unwrap();
    assert!(
        document
            .object(a)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&after)
    );
}

#[test]
fn catalog_edits_that_break_live_instances_or_fail_a_late_placement_are_atomic() {
    let mut document = Document::default();
    let part = document
        .add_block_definition("part", vec![leaf(&document, 1.)])
        .unwrap();
    let a = document.add_block_instance(placement(part, 0.)).unwrap();
    let scale =
        AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.).unwrap(), 2.).unwrap();
    document
        .add_block_instance(BlockReference::try_new(part, scale).unwrap())
        .unwrap();
    document.add_geometry(point(3.)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    assert!(document.remove_block_definition(part).is_err());
    assert!(document.set_block_definitions(vec![]).is_err());
    assert!(
        document
            .replace_block_definition_members(part, vec![])
            .is_err()
    );
    assert!(
        document
            .replace_block_definition_members(part, vec![leaf(&document, f64::MAX)])
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
    assert_eq!(x(&document, a), 1.);
    // A catalog rollback also exchanges every refreshed instance cache.
    let snapshot = document.object(a).unwrap().geometry_snapshot().clone();
    document.begin_transaction("edit definition").unwrap();
    document
        .replace_block_definition_members(part, vec![leaf(&document, 4.)])
        .unwrap();
    document.rollback_transaction().unwrap();
    assert!(
        document
            .object(a)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&snapshot)
    );
    assert_eq!(x(&document, a), 1.);
}

#[test]
fn transformed_copies_keep_definition_identity_attributes_and_group_history() {
    let mut document = Document::default();
    let part = document
        .add_block_definition("part", vec![leaf(&document, 2.)])
        .unwrap();
    let object = document.add_block_instance(placement(part, 10.)).unwrap();
    document
        .set_object_geometry_user_text([object], "code", Some("original"))
        .unwrap();
    document
        .add_group(Some("assembly".into()), [object])
        .unwrap();
    let reflection = AffineTransform3::try_new(
        [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        Vector3::try_new(0., 0., 0.).unwrap(),
    )
    .unwrap();
    let copied = document
        .copy_objects_transformed([object], reflection)
        .unwrap()[0];
    assert_eq!(instance(&document, copied).reference().definition(), part);
    assert_eq!(x(&document, copied), -12.);
    assert_ne!(
        document.object(copied).unwrap().top_group(),
        document.object(object).unwrap().top_group()
    );
    assert_eq!(
        document.object(copied).unwrap().geometry_user_text()["code"],
        "original"
    );
    document
        .replace_block_definition_members(part, vec![leaf(&document, 3.)])
        .unwrap();
    assert_eq!([x(&document, object), x(&document, copied)], [13., -13.]);
    document.undo().unwrap();
    document.undo().unwrap();
    assert!(document.object(copied).is_none());
    document.redo().unwrap();
    assert_eq!(x(&document, copied), -12.);
}

#[test]
fn stale_or_foreign_instance_values_are_revalidated_before_admission_and_replacement() {
    let mut document = Document::default();
    let part = document
        .add_block_definition("part", vec![leaf(&document, 1.)])
        .unwrap();
    let value = document
        .block_instance_geometry(placement(part, 10.))
        .unwrap();
    let object = document.add_geometry(point(2.)).unwrap();
    document
        .replace_block_definition_members(part, vec![leaf(&document, 4.)])
        .unwrap();
    document
        .replace_object_geometries([(object, value.clone())])
        .unwrap();
    assert_eq!(x(&document, object), 14.);
    let copy = document.add_geometry(value.clone()).unwrap();
    assert_eq!(x(&document, copy), 14.);
    let definition = BlockDefinition::new(
        "illegal member",
        vec![BlockMember::new(
            BlockContent::Geometry(value.clone().into()),
            ObjectAttributes::on_layer(document.current_layer_id()),
        )],
    );
    let before = format!("{document:?}");
    assert!(document.set_block_definitions(vec![definition]).is_err());
    assert_eq!(format!("{document:?}"), before);
    let mut foreign = Document::default();
    let ordinary = foreign.add_geometry(point(7.)).unwrap();
    let before = format!("{foreign:?}");
    assert!(foreign.add_geometry(value.clone()).is_err());
    assert!(
        foreign
            .replace_object_geometries([(ordinary, value.clone())])
            .is_err()
    );
    assert!(
        foreign
            .copy_object_geometries_into_source_groups([(ordinary, value)])
            .is_err()
    );
    assert_eq!(format!("{foreign:?}"), before);
}

#[test]
fn nested_instance_unit_changes_scale_every_translation_once_and_replay_storage() {
    let mut document = Document::default();
    let child = document
        .add_block_definition("child", vec![leaf(&document, 1000.)])
        .unwrap();
    let child_map = AffineTransform3::try_new(
        [[2., 0., 0.], [0., 3., 0.], [0., 0., 4.]],
        Vector3::try_new(100., 200., 300.).unwrap(),
    )
    .unwrap();
    let root = document
        .add_block_definition(
            "root",
            vec![BlockMember::new(
                BlockContent::Reference(BlockReference::try_new(child, child_map).unwrap()),
                ObjectAttributes::on_layer(document.current_layer_id()),
            )],
        )
        .unwrap();
    let root_map = AffineTransform3::try_new(
        [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
        Vector3::try_new(10000., 20000., 30000.).unwrap(),
    )
    .unwrap();
    let object = document
        .add_block_instance(BlockReference::try_new(root, root_map).unwrap())
        .unwrap();
    let original = document.object(object).unwrap().geometry_snapshot().clone();
    document.set_units(LengthUnitSystem::Meters, true).unwrap();
    let state = instance(&document, object);
    assert_eq!(
        state.reference().transform().linear_rows(),
        root_map.linear_rows()
    );
    assert_eq!(
        state.reference().transform().translation().to_array(),
        [10., 20., 30.]
    );
    let Geometry::Point(p) = &*state.members()[0].geometry else {
        panic!()
    };
    assert_eq!(p.to_array(), [9.8, 22.1, 30.3]);
    let scaled = document.object(object).unwrap().geometry_snapshot().clone();
    document.undo().unwrap();
    assert!(
        document
            .object(object)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&original)
    );
    document.redo().unwrap();
    assert!(
        document
            .object(object)
            .unwrap()
            .geometry_snapshot()
            .shares_storage_with(&scaled)
    );
    document
        .set_units(LengthUnitSystem::Millimeters, true)
        .unwrap();
    assert_eq!(document.object(object).unwrap().geometry(), &*original);
}

#[test]
fn unit_and_singular_transform_failures_preserve_catalog_objects_and_redo() {
    let mut document = Document::default();
    let ordinary = document.add_geometry(point(1.)).unwrap();
    let part = document
        .add_block_definition("part", vec![leaf(&document, 0.)])
        .unwrap();
    let object = document
        .add_block_instance(placement(part, f64::MAX))
        .unwrap();
    document.add_geometry(point(3.)).unwrap();
    document.undo().unwrap();
    let before = format!("{document:?}");
    assert!(
        document
            .set_units(
                LengthUnitSystem::Custom {
                    name: "half mm".into(),
                    meters_per_unit: 0.0005
                },
                true
            )
            .is_err()
    );
    let collapse =
        AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.).unwrap(), 0.).unwrap();
    assert!(
        document
            .transform_objects([ordinary, object], collapse)
            .is_err()
    );
    assert_eq!(format!("{document:?}"), before);
}

#[test]
fn hierarchical_display_resolves_parent_color_visibility_and_lock_without_changing_raw_metadata() {
    let mut document = Document::default();
    let layer = document
        .add_layer("members", ColorRgb::new(5, 10, 15))
        .unwrap();
    let raw = ObjectAttributes::on_layer(layer).with_color_source(ObjectColorSource::Parent);
    let child = document
        .add_block_definition(
            "child",
            vec![BlockMember::new(
                BlockContent::Geometry(point(1.).into()),
                raw.clone(),
            )],
        )
        .unwrap();
    let parent_attrs = ObjectAttributes::on_layer(document.current_layer_id())
        .with_object_color(ColorRgb::new(50, 60, 70));
    let root = document
        .add_block_definition(
            "root",
            vec![BlockMember::new(
                BlockContent::Reference(placement(child, 0.)),
                parent_attrs,
            )],
        )
        .unwrap();
    let object = document
        .add_block_instance_with_attributes(
            placement(root, 0.),
            ObjectAttributes::on_layer(document.current_layer_id())
                .with_object_color(ColorRgb::new(100, 110, 120)),
        )
        .unwrap();
    let path = instance(&document, object).members()[0].path.clone();
    let state = document
        .block_member_display(document.object(object).unwrap().attributes(), &path)
        .unwrap();
    assert_eq!(
        state,
        BlockMemberDisplay {
            color: ColorRgb::new(50, 60, 70),
            visible: true,
            locked: false
        }
    );
    document.set_layer_locked(layer, true).unwrap();
    assert!(
        !document
            .block_member_display(document.object(object).unwrap().attributes(), &path)
            .unwrap()
            .locked
    );
    document.set_layer_visibility(layer, false).unwrap();
    assert!(
        !document
            .block_member_display(document.object(object).unwrap().attributes(), &path)
            .unwrap()
            .visible
    );
    assert_eq!(instance(&document, object).members()[0].attributes, raw);
}
