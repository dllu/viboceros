use super::*;

#[test]
fn crease_policy_is_shared_across_documents_and_separate_from_model_history() {
    let registry = CommandRegistry::with_builtins();
    let mut first = Document::default();
    let mut second = Document::default();
    assert!(registry.crease_splitting());
    registry.execute(&mut first, "Point 1,2,3").unwrap();
    registry.execute(&mut first, "Undo").unwrap();
    registry
        .execute(&mut first, "CreaseSplitting _Disable")
        .unwrap();
    assert!(!registry.crease_splitting());
    assert_eq!(first.redo_label(), Some("Point"));
    assert_eq!(first.undo_label(), None);
    registry
        .execute(&mut second, "CreaseSplitting Toggle")
        .unwrap();
    assert!(registry.crease_splitting());
    assert_eq!(second.undo_label(), None);
    for command in [
        "CreaseSplitting",
        "CreaseSplitting No",
        "CreaseSplitting Disable Enable",
    ] {
        assert!(registry.execute(&mut first, command).is_err());
        assert!(registry.crease_splitting());
        assert_eq!(first.redo_label(), Some("Point"));
    }
    registry
        .execute(&mut first, "CreaseSplitting Disable")
        .unwrap();
    registry.execute(&mut first, "Redo").unwrap();
    assert!(!registry.crease_splitting());
    assert_eq!(first.objects().len(), 1);
    assert!(CommandRegistry::with_builtins().crease_splitting());
}
