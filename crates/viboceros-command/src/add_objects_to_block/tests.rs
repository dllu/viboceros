use super::*;
fn fixture() -> (Document, CommandRegistry, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 1,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry.execute(&mut doc, "Block 0,0,0 part").unwrap();
    let target = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Point 7,0,0").unwrap();
    let added = doc.objects().last().unwrap().id();
    (doc, registry, target, added)
}
#[test]
fn selected_and_explicit_sources_share_one_transactional_command() {
    let (mut doc, registry, target, added) = fixture();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    doc.select_objects_direct([target, added], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, &format!("AddObjectsToBlock {target}"))
        .unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(doc.undo_label(), Some("AddObjectsToBlock"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    registry
        .execute(&mut doc, &format!("AddObjectsToBlock {target} {added}"))
        .unwrap();
    assert!(doc.object(added).is_none());
}
#[test]
fn malformed_target_or_sources_do_not_edit_the_model() {
    let (mut doc, registry, target, added) = fixture();
    let before = format!("{doc:?}");
    for text in [
        "AddObjectsToBlock".to_owned(),
        "AddObjectsToBlock invalid".to_owned(),
        format!("AddObjectsToBlock {target} invalid"),
        format!("AddObjectsToBlock {added} {target}"),
    ] {
        assert!(registry.execute(&mut doc, &text).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}
