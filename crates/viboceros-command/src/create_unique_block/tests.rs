use super::*;

#[test]
fn selected_roots_share_one_new_definition_and_original_peers_remain_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Point 1,2,3").unwrap();
    doc.select_all();
    registry.execute(&mut doc, "Block 0,0,0 part").unwrap();
    let original = doc.objects().next().unwrap().id();
    registry.execute(&mut doc, "Insert part 10,0,0").unwrap();
    let chosen = doc.objects().last().unwrap().id();
    doc.select_objects_direct([chosen], SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut doc, "CreateUniqueBlock \"Part B\"")
        .unwrap();
    let definition = doc.block_definition_by_name("Part B").unwrap().id();
    assert!(
        matches!(doc.object(chosen).unwrap().geometry(),Geometry::BlockInstance(i) if i.reference().definition()==definition)
    );
    assert!(
        matches!(doc.object(original).unwrap().geometry(),Geometry::BlockInstance(i) if i.reference().definition()!=definition)
    );
    assert!(doc.is_selected(chosen));
    assert_eq!(doc.undo_label(), Some("CreateUniqueBlock"));
    registry.execute(&mut doc, "Undo").unwrap();
    assert!(doc.block_definition_by_name("Part B").is_none());
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(
        doc.block_definition_by_name("Part B").unwrap().id(),
        definition
    );
}

#[test]
fn wrong_names_and_mixed_definitions_fail_without_consuming_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for name in ["a", "b"] {
        doc.clear_selection();
        registry.execute(&mut doc, "Point 1,2,3").unwrap();
        let id = doc.objects().last().unwrap().id();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(&mut doc, &format!("Block 0,0,0 {name}"))
            .unwrap();
    }
    doc.select_all();
    let before = format!("{doc:?}");
    assert!(registry.execute(&mut doc, "CreateUniqueBlock c").is_err());
    assert_eq!(format!("{doc:?}"), before);
    let first = doc.objects().next().unwrap().id();
    doc.select_objects_direct([first], SelectionMode::Replace)
        .unwrap();
    assert!(registry.execute(&mut doc, "CreateUniqueBlock a").is_err());
}
