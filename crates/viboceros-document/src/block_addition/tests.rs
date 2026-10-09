use super::*;
use viboceros_geometry::{Point3, Vector3};
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn snapshot(doc: &Document) -> String {
    format!(
        "{:?}",
        (
            doc.objects().collect::<Vec<_>>(),
            doc.block_definitions().collect::<Vec<_>>(),
            doc.groups().collect::<Vec<_>>(),
            doc.selected_object_ids().collect::<Vec<_>>()
        )
    )
}
fn fixture() -> (Document, BlockDefinitionId, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(point(1., 0., 0.)))
        .unwrap();
    let (definition, root) = doc
        .create_block_from_objects("part", point(0., 0., 0.), [id])
        .unwrap();
    let added = doc
        .add_geometry(Geometry::Point(point(12., 3., 4.)))
        .unwrap();
    (doc, definition, root, added)
}
#[test]
fn affine_addition_consumes_sources_keeps_metadata_and_restores_all_history() {
    let (mut doc, definition, root, added) = fixture();
    let transform = AffineTransform3::try_new(
        [[0., -2., 0.], [3., 0., 0.], [0., 0., -4.]],
        Vector3::try_new(10., 20., 30.).unwrap(),
    )
    .unwrap();
    let target = doc
        .add_block_instance(BlockReference::try_new(definition, transform).unwrap())
        .unwrap();
    doc.set_object_geometry_user_text([added], "Shape", Some("keep"))
        .unwrap();
    doc.set_object_user_text([added], "Part", Some("member"))
        .unwrap();
    doc.add_group(None, [added, root]).unwrap();
    doc.set_objects_locked([root], true).unwrap();
    let before = snapshot(&doc);
    assert_eq!(doc.add_objects_to_block(target, [added]).unwrap(), 1);
    assert!(doc.object(added).is_none());
    assert_eq!(doc.block_definition(definition).unwrap().members().len(), 2);
    let member = &doc.block_definition(definition).unwrap().members()[1];
    assert!(member.group_ids().is_empty());
    assert_eq!(
        member.geometry_user_text().get("Shape").map(String::as_str),
        Some("keep")
    );
    assert_eq!(
        member
            .attributes()
            .user_text()
            .get("Part")
            .map(String::as_str),
        Some("member")
    );
    let Geometry::BlockInstance(instance) = doc.object(target).unwrap().geometry() else {
        panic!()
    };
    let Geometry::Point(p) = &*instance.members()[1].geometry else {
        panic!()
    };
    assert!(p.distance_to(point(12., 3., 4.)).unwrap() < 1e-12);
    assert!(doc.object(root).unwrap().attributes().is_locked());
    assert_eq!(doc.undo_label(), Some("AddObjectsToBlock"));
    let after = snapshot(&doc);
    doc.undo().unwrap();
    assert_eq!(snapshot(&doc), before);
    doc.redo().unwrap();
    assert_eq!(snapshot(&doc), after);
}
#[test]
fn nested_addition_keeps_references_and_updates_ancestors_without_flattening() {
    let (mut doc, definition, root, added) = fixture();
    let (child, child_root) = doc
        .create_block_from_objects("child", point(0., 0., 0.), [added])
        .unwrap();
    let (parent, _) = doc
        .create_block_from_objects("parent", point(0., 0., 0.), [root])
        .unwrap();
    let target = doc
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    doc.add_objects_to_block(target, [child_root]).unwrap();
    assert!(
        matches!(doc.block_definition(definition).unwrap().members()[1].content(),BlockContent::Reference(r)if r.definition()==child)
    );
    let resolved = doc
        .resolve_block(BlockReference::try_new(parent, AffineTransform3::identity()).unwrap())
        .unwrap();
    assert_eq!(resolved.len(), 2);
}
#[test]
fn cycles_missing_protected_and_empty_sources_preserve_model_and_redo() {
    let (mut doc, _, root, added) = fixture();
    let (_, parent) = doc
        .create_block_from_objects("parent", point(0., 0., 0.), [root])
        .unwrap();
    let definition = doc.block_definition_by_name("part").unwrap().id();
    let target = doc
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    let extra = doc
        .add_geometry(Geometry::Point(point(3., 0., 0.)))
        .unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for ids in [vec![target], vec![parent], vec![extra], Vec::new()] {
        assert!(doc.add_objects_to_block(target, ids).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    doc.redo().unwrap();
    doc.set_objects_locked([added], true).unwrap();
    let before = format!("{doc:?}");
    assert!(doc.add_objects_to_block(target, [added]).is_err());
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn addition_clears_existing_prototype_groups_but_keeps_model_groups_and_undo() {
    let mut doc = Document::default();
    let first = doc
        .add_geometry(Geometry::Point(point(1., 0., 0.)))
        .unwrap();
    let added = doc
        .add_geometry(Geometry::Point(point(7., 0., 0.)))
        .unwrap();
    doc.add_group(None, [first, added]).unwrap();
    let (definition, target) = doc
        .create_block_from_objects("part", point(0., 0., 0.), [first])
        .unwrap();
    assert!(
        !doc.block_definition(definition).unwrap().members()[0]
            .group_ids()
            .is_empty()
    );
    let before = snapshot(&doc);
    doc.add_objects_to_block(target, [added]).unwrap();
    assert!(
        doc.block_definition(definition)
            .unwrap()
            .members()
            .iter()
            .all(|m| m.group_ids().is_empty())
    );
    assert_eq!(doc.groups().len(), 2);
    doc.undo().unwrap();
    assert_eq!(snapshot(&doc), before);
}

#[test]
fn native_conic_addition_fits_a_circle_and_preserves_domain_without_changing_general_affine_editing()
 {
    let (mut doc, definition, _, _) = fixture();
    let placement = AffineTransform3::try_new(
        [[0., -2., 0.], [3., 0., 0.], [0., 0., -4.]],
        Vector3::try_new(10., 20., 30.).unwrap(),
    )
    .unwrap();
    let target = doc
        .add_block_instance(BlockReference::try_new(definition, placement).unwrap())
        .unwrap();
    let circle = viboceros_geometry::Circle3::try_from_frame(
        point(11., 22., 33.),
        2.,
        Vector3::try_new(1., 0., 0.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::NUMERICAL_VALIDATION,
    )
    .unwrap();
    let added = doc.add_geometry(Geometry::Circle(circle)).unwrap();
    let inverse = placement.try_inverse().unwrap();
    let general = Geometry::Circle(circle)
        .transformed(inverse, Tolerance::NUMERICAL_VALIDATION)
        .unwrap();
    assert!(matches!(general, Geometry::NurbsCurve(_)));
    doc.add_objects_to_block(target, [added]).unwrap();
    let BlockContent::Geometry(geometry) =
        doc.block_definition(definition).unwrap().members()[1].content()
    else {
        panic!()
    };
    let Geometry::Circle(fitted) = &**geometry else {
        panic!()
    };
    assert!((fitted.radius() - 2. / 6_f64.sqrt()).abs() < 1e-15);
    assert_eq!(fitted.domain(), circle.domain());
    let ideal = inverse
        .transform_point(circle.point_at_angle(0.).unwrap())
        .unwrap();
    assert!(
        fitted
            .evaluate(*fitted.domain().start())
            .unwrap()
            .distance_to(ideal)
            .unwrap()
            > 0.1
    );
}
