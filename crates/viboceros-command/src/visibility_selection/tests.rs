use super::*;

#[test]
fn show_selected_changes_only_requested_hidden_objects_and_is_undoable() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for x in 0..3 {
        registry
            .execute(&mut document, &format!("Point {x},0,0"))
            .unwrap();
    }
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .set_objects_visibility([ids[0], ids[1]], false)
        .unwrap();
    let prompt = registry
        .object_selection_prompt("ShowSelected")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::HiddenObjects);
    assert_eq!(
        registry
            .execute(&mut document, &format!("ShowSelected Ids={}", ids[1]))
            .unwrap(),
        "Showed 1 object(s)"
    );
    assert!(!document.object(ids[0]).unwrap().attributes().is_visible());
    assert!(document.object(ids[1]).unwrap().attributes().is_visible());
    assert!(document.object(ids[2]).unwrap().attributes().is_visible());
    registry.execute(&mut document, "Undo").unwrap();
    assert!(!document.object(ids[1]).unwrap().attributes().is_visible());
    registry.execute(&mut document, "Redo").unwrap();
    assert!(document.object(ids[1]).unwrap().attributes().is_visible());
}

#[test]
fn unlock_selected_preflights_all_ids_before_mutation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .set_objects_locked(ids.iter().copied(), true)
        .unwrap();
    let prompt = registry
        .object_selection_prompt("UnlockSelected")
        .unwrap()
        .unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::LockedObjects);
    assert!(
        registry
            .execute(
                &mut document,
                &format!("UnlockSelected Ids={},99999", ids[0])
            )
            .is_err()
    );
    assert!(document.object(ids[0]).unwrap().attributes().is_locked());
    assert_eq!(
        registry
            .execute(&mut document, &format!("UnlockSelected Ids={}", ids[0]))
            .unwrap(),
        "Unlocked 1 object(s)"
    );
    assert!(!document.object(ids[0]).unwrap().attributes().is_locked());
    assert!(document.object(ids[1]).unwrap().attributes().is_locked());
    registry.execute(&mut document, "Undo").unwrap();
    assert!(document.object(ids[0]).unwrap().attributes().is_locked());
}
