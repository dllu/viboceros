use super::*;
use viboceros_document::BlockReference;

fn fixture() -> (Document, ObjectId) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.select_all();
    registry.execute(&mut doc, "Block 0,0,0 child").unwrap();
    let child = doc.objects().next().unwrap().id();
    doc.select_object(child, SelectionMode::Replace).unwrap();
    registry.execute(&mut doc, "Block 10,0,0 parent").unwrap();
    let root = doc.objects().next().unwrap().id();
    doc.select_object(root, SelectionMode::Replace).unwrap();
    (doc, root)
}

#[test]
fn explode_keeps_nested_instances_and_explode_block_recursively_returns_geometry() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, root) = fixture();
    let before = doc.object(root).unwrap().clone();
    registry.execute(&mut doc, "Explode").unwrap();
    assert!(matches!(
        doc.selected_objects().next().unwrap().geometry(),
        Geometry::BlockInstance(_)
    ));
    assert!(doc.object(root).is_none());
    assert_eq!(doc.undo_label(), Some("Explode"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(root).unwrap().geometry(), before.geometry());
    doc.select_object(root, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut doc, "ExplodeBlock GroupOutput=Yes")
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    let object = doc.objects().next().unwrap();
    assert_eq!(
        object.geometry(),
        &Geometry::Point(Point3::try_new(1., 2., 3.).unwrap())
    );
    assert!(object.top_group().is_some());
    assert!(doc.is_selected(object.id()));
    assert_eq!(doc.block_definitions().len(), 2);
    registry.execute(&mut doc, "Undo").unwrap();
    assert!(doc.object(root).is_some());
}

#[test]
fn mixed_ordinary_explode_and_block_expansion_share_one_transaction() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, root) = fixture();
    registry
        .execute(&mut doc, "Polyline 0,0,0 2,0,0 2,2,0")
        .unwrap();
    let polyline = doc.objects().last().unwrap().id();
    doc.select_objects_direct([root, polyline], SelectionMode::Replace)
        .unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut doc, "Explode").unwrap();
    assert_eq!(doc.objects().len(), 3);
    assert_eq!(doc.selected_object_count(), 3);
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn all_blocks_uses_every_root_and_bad_options_or_limit_failures_preserve_redo() {
    let registry = CommandRegistry::with_builtins();
    let (mut doc, _) = fixture();
    doc.clear_selection();
    registry
        .execute(&mut doc, "ExplodeBlock AllBlocks")
        .unwrap();
    assert!(
        doc.objects()
            .all(|object| !matches!(object.geometry(), Geometry::BlockInstance(_)))
    );
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    for input in [
        "ExplodeBlock GroupOutput=Maybe",
        "ExplodeBlock AllBlocks AllBlocks=Yes",
        "ExplodeBlock Unknown=Yes",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
    let definition = doc.block_definition_by_name("parent").unwrap().id();
    let a = doc
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    let b = doc
        .add_block_instance(
            BlockReference::try_new(definition, AffineTransform3::identity()).unwrap(),
        )
        .unwrap();
    doc.select_objects_direct([a, b], SelectionMode::Replace)
        .unwrap();
    let before = format!("{doc:?}");
    assert!(run_with_budget(&mut doc, vec![a, b], false, 1).is_err());
    assert_eq!(format!("{doc:?}"), before);
}
