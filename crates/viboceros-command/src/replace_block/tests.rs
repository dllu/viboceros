use super::*;
fn fixture() -> (Document, CommandRegistry, ObjectId, ObjectId) {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    for (name, x) in [("original", 1), ("target", 7)] {
        doc.clear_selection();
        registry
            .execute(&mut doc, &format!("Point {x},0,0"))
            .unwrap();
        let id = doc.objects().last().unwrap().id();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut doc, &format!("Block 0,0,0 {name}"))
            .unwrap();
    }
    let original = doc.objects().next().unwrap().id();
    registry
        .execute(&mut doc, "Insert original 10,0,0")
        .unwrap();
    let chosen = doc.objects().last().unwrap().id();
    doc.select_objects_direct([chosen], SelectionMode::Replace)
        .unwrap();
    (doc, registry, original, chosen)
}
#[test]
fn none_changes_selected_root_and_all_changes_original_peers_with_history() {
    let (mut doc, registry, original, chosen) = fixture();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut doc, "ReplaceBlock None BlockDefinitionName=target")
        .unwrap();
    let target = doc.block_definition_by_name("target").unwrap().id();
    assert!(
        matches!(doc.object(chosen).unwrap().geometry(),Geometry::BlockInstance(i)if i.reference().definition()==target)
    );
    assert!(
        matches!(doc.object(original).unwrap().geometry(),Geometry::BlockInstance(i)if i.reference().definition()!=target)
    );
    assert_eq!(doc.undo_label(), Some("ReplaceBlock"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    doc.select_objects_direct([chosen], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, "ReplaceBlock All target")
        .unwrap();
    assert!(
        matches!(doc.object(original).unwrap().geometry(),Geometry::BlockInstance(i)if i.reference().definition()==target)
    );
}
#[test]
fn duplicate_scope_or_target_and_missing_definitions_leave_state_unchanged() {
    let (mut doc, registry, _, _) = fixture();
    let before = format!("{doc:?}");
    for input in [
        "ReplaceBlock All None target",
        "ReplaceBlock target original",
        "ReplaceBlock missing",
        "ReplaceBlock Unexpected=target",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(format!("{doc:?}"), before);
    }
}
