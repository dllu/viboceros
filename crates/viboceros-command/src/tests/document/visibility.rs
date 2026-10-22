//! document visibility command regressions.
use super::*;

#[test]
fn hide_show_lock_and_unlock_preserve_identity_groups_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    registry.execute(&mut document, "Point 2,0,0").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    document.select_object(ids[1], SelectionMode::Add).unwrap();
    registry.execute(&mut document, "Group Pair").unwrap();
    let group = document.group_by_name("Pair").unwrap().id();
    document.clear_selection();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(document.selected_object_count(), 2);

    assert_eq!(
        registry.execute(&mut document, "Hide").unwrap(),
        "Hid 2 object(s)"
    );
    assert_eq!(document.undo_label(), Some("Hide"));
    assert_eq!(document.selected_object_count(), 0);
    for id in &ids[..2] {
        let object = document.object(*id).unwrap();
        assert_eq!(object.id(), *id);
        assert!(!object.attributes().is_visible());
        assert!(!document.is_object_selectable(*id));
    }
    assert_eq!(document.group(group).unwrap().members().len(), 2);
    assert!(document.object(ids[2]).unwrap().attributes().is_visible());

    document
        .select_object(ids[2], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Show").unwrap(),
        "Showed 2 object(s)"
    );
    assert_eq!(document.undo_label(), Some("Show"));
    assert!(document.is_selected(ids[2]));
    assert!(
        ids[..2]
            .iter()
            .all(|id| document.object(*id).unwrap().attributes().is_visible())
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert!(
        ids[..2]
            .iter()
            .all(|id| !document.object(*id).unwrap().attributes().is_visible())
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert!(
        ids[..2]
            .iter()
            .all(|id| document.object(*id).unwrap().attributes().is_visible())
    );

    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "Lock").unwrap(),
        "Locked 2 object(s)"
    );
    assert_eq!(document.undo_label(), Some("Lock"));
    assert_eq!(document.selected_object_count(), 0);
    for id in &ids[..2] {
        assert!(document.object(*id).unwrap().attributes().is_locked());
        assert_eq!(
            document.select_object(*id, SelectionMode::Replace),
            Err(DocumentError::ObjectNotSelectable(*id))
        );
    }
    assert_eq!(
        registry.execute(&mut document, "Unlock").unwrap(),
        "Unlocked 2 object(s)"
    );
    assert_eq!(document.undo_label(), Some("Unlock"));
    assert!(
        ids[..2]
            .iter()
            .all(|id| !document.object(*id).unwrap().attributes().is_locked())
    );
    registry.execute(&mut document, "Undo").unwrap();
    assert!(
        ids[..2]
            .iter()
            .all(|id| document.object(*id).unwrap().attributes().is_locked())
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert!(
        ids[..2]
            .iter()
            .all(|id| !document.object(*id).unwrap().attributes().is_locked())
    );

    let history = document.undo_label().map(str::to_owned);
    assert_eq!(
        registry.execute(&mut document, "Show").unwrap(),
        "Showed 0 object(s)"
    );
    assert_eq!(
        registry.execute(&mut document, "Unlock").unwrap(),
        "Unlocked 0 object(s)"
    );
    assert_eq!(document.undo_label(), history.as_deref());
    assert!(matches!(
        registry.execute(&mut document, "Hide unexpected"),
        Err(CommandError::Usage("Hide"))
    ));
    document.clear_selection();
    assert!(matches!(
        registry.execute(&mut document, "Lock"),
        Err(CommandError::NoObjectsSelected)
    ));
    assert_eq!(document.undo_label(), history.as_deref());
    assert_eq!(document.group(group).unwrap().members().len(), 2);
}

#[test]
fn hide_swap_and_lock_swap_are_reversible_three_mode_involutions() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    registry.execute(&mut document, "Point 2,0,0").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document.set_objects_visibility([ids[1]], false).unwrap();
    document.set_objects_locked([ids[2]], true).unwrap();
    let modes = |document: &Document| {
        ids.iter()
            .map(|id| {
                let attributes = document.object(*id).unwrap().attributes();
                if !attributes.is_visible() {
                    "hidden"
                } else if attributes.is_locked() {
                    "locked"
                } else {
                    "normal"
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);

    assert_eq!(
        registry.execute(&mut document, "HideSwap").unwrap(),
        "Swapped hidden state on 2 object(s)"
    );
    assert_eq!(document.undo_label(), Some("HideSwap"));
    assert_eq!(modes(&document), vec!["hidden", "normal", "locked"]);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(modes(&document), vec!["hidden", "normal", "locked"]);
    registry.execute(&mut document, "HideSwap").unwrap();
    assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);

    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    assert_eq!(
        registry.execute(&mut document, "LockSwap").unwrap(),
        "Swapped lock state on 2 object(s)"
    );
    assert_eq!(document.undo_label(), Some("LockSwap"));
    assert_eq!(document.selected_object_count(), 0);
    assert_eq!(modes(&document), vec!["locked", "hidden", "normal"]);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(modes(&document), vec!["locked", "hidden", "normal"]);
    registry.execute(&mut document, "LockSwap").unwrap();
    assert_eq!(modes(&document), vec!["normal", "hidden", "locked"]);

    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "HideSwap unexpected"),
        Err(CommandError::Usage("HideSwap"))
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), history.as_deref());
}

#[test]
fn isolate_commands_restore_only_their_own_object_modes() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for x in 0..4 {
        registry
            .execute(&mut document, &format!("Point {x},0,0"))
            .unwrap();
    }
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    document.set_objects_visibility([ids[2]], false).unwrap();
    document.set_objects_locked([ids[3]], true).unwrap();
    document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    let modes = |document: &Document| {
        ids.iter()
            .map(|id| {
                let attributes = document.object(*id).unwrap().attributes();
                if !attributes.is_visible() {
                    "hidden"
                } else if attributes.is_locked() {
                    "locked"
                } else {
                    "normal"
                }
            })
            .collect::<Vec<_>>()
    };
    let initial = vec!["normal", "normal", "hidden", "locked"];

    assert_eq!(
        registry.execute(&mut document, "Isolate").unwrap(),
        "Isolated 1 object(s)"
    );
    assert_eq!(document.undo_label(), Some("Isolate"));
    assert_eq!(document.isolated_hidden_object_count(), 1);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
    assert_eq!(
        modes(&document),
        vec!["normal", "hidden", "hidden", "locked"]
    );
    assert_eq!(
        registry.execute(&mut document, "Isolate").unwrap(),
        "Isolated 0 object(s)"
    );
    assert_eq!(document.undo_label(), Some("Isolate"));
    assert_eq!(
        registry.execute(&mut document, "Unisolate").unwrap(),
        "Unisolated 1 object(s)"
    );
    assert_eq!(modes(&document), initial);
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.isolated_hidden_object_count(), 1);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(modes(&document), initial);

    assert_eq!(
        registry.execute(&mut document, "IsolateLock").unwrap(),
        "Locked 1 non-selected object(s)"
    );
    assert_eq!(document.undo_label(), Some("IsolateLock"));
    assert_eq!(document.isolated_locked_object_count(), 1);
    assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [ids[0]]);
    assert_eq!(
        modes(&document),
        vec!["normal", "locked", "hidden", "locked"]
    );
    assert_eq!(
        registry.execute(&mut document, "UnisolateLock").unwrap(),
        "Unlocked 1 isolated object(s)"
    );
    assert_eq!(modes(&document), initial);

    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "Unisolate extra"),
        Err(CommandError::Usage("Unisolate"))
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), history.as_deref());
    document.clear_selection();
    assert!(matches!(
        registry.execute(&mut document, "Isolate"),
        Err(CommandError::NoObjectsSelected)
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), history.as_deref());
}
