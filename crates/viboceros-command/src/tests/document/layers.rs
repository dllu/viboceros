//! document layers command regressions.
use super::*;

#[test]
fn undo_and_redo_commands_replay_a_model_edit() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 1,2,3").unwrap();
    assert_eq!(document.objects().len(), 1);

    assert_eq!(
        registry.execute(&mut document, "Undo").unwrap(),
        "Undid Point"
    );
    assert_eq!(document.objects().len(), 0);
    assert_eq!(
        registry.execute(&mut document, "Redo").unwrap(),
        "Redid Point"
    );
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn compound_layer_command_is_one_undo_step() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let default_layer = document.current_layer_id();
    registry
        .execute(&mut document, "Layer Construction")
        .unwrap();
    assert_eq!(document.layers().len(), 2);
    assert_ne!(document.current_layer_id(), default_layer);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.layers().len(), 1);
    assert_eq!(document.current_layer_id(), default_layer);
}

#[test]
fn layer_command_manages_named_state_and_preserves_undo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry
        .execute(&mut document, "Layer New Construction")
        .unwrap();
    registry
        .execute(
            &mut document,
            "Layer Rename Construction => Reference Geometry",
        )
        .unwrap();
    registry
        .execute(&mut document, "Layer Color 12,34,56 Reference Geometry")
        .unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry
        .execute(&mut document, "Layer Hide Reference Geometry")
        .unwrap();

    let layer = document.layer_by_name("reference geometry").unwrap();
    assert_eq!(layer.color(), ColorRgb::new(12, 34, 56));
    assert!(!layer.is_visible());
    registry.execute(&mut document, "Undo").unwrap();
    assert!(
        document
            .layer_by_name("Reference Geometry")
            .unwrap()
            .is_visible()
    );

    registry
        .execute(&mut document, "Layer Lock Reference Geometry")
        .unwrap();
    assert!(
        registry
            .execute(&mut document, "Layer Current Reference Geometry")
            .is_err()
    );
    registry
        .execute(&mut document, "Layer Unlock Reference Geometry")
        .unwrap();
    registry
        .execute(&mut document, "Layer Current Reference Geometry")
        .unwrap();
}

#[test]
fn change_and_copy_to_layer_preserve_rhino_identity_selection_and_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let default = document.current_layer_id();
    registry
        .execute(&mut document, "Layer New Target Parts")
        .unwrap();
    let target = document.layer_by_name("Target Parts").unwrap().id();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Assembly").unwrap();
    let original_ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();

    assert_eq!(
        registry
            .execute(&mut document, "ChangeLayer target parts")
            .unwrap(),
        "Changed 1 object(s) to layer 'target parts'"
    );
    assert_eq!(document.undo_label(), Some("ChangeLayer"));
    assert_eq!(document.current_layer_id(), default);
    assert_eq!(document.selected_object_count(), 2);
    assert!(
        original_ids
            .iter()
            .all(|id| { document.object(*id).unwrap().attributes().layer_id() == target })
    );
    assert_eq!(
        document.group_by_name("Assembly").unwrap().members().len(),
        2
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "CopyToLayer Target Parts")
            .unwrap(),
        "Copied 1 object(s) to layer 'Target Parts'"
    );
    assert_eq!(document.undo_label(), Some("CopyToLayer"));
    assert_eq!(document.objects().len(), 3);
    assert_eq!(document.selected_object_count(), 2);
    assert!(original_ids.iter().all(|id| document.is_selected(*id)));
    let copy = document
        .objects()
        .find(|object| !original_ids.contains(&object.id()))
        .unwrap();
    assert_eq!(copy.attributes().layer_id(), target);
    assert!(!document.is_selected(copy.id()));
    assert_eq!(
        document.group_by_name("Group01").unwrap().members().len(),
        1
    );
    assert_eq!(document.current_layer_id(), default);

    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().len(), 2);
    assert!(document.group_by_name("Group01").is_none());
    document.clear_selection();
    assert!(matches!(
        registry.execute(&mut document, "ChangeLayer Target Parts"),
        Err(CommandError::NoObjectsSelected)
    ));
}

#[test]
fn layer_delete_and_color_validation_are_safe() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(
        registry
            .execute(&mut document, "Layer Color 256,0,0 Default")
            .is_err()
    );
    assert_eq!(
        document.layer(document.current_layer_id()).unwrap().color(),
        ColorRgb::BLACK
    );

    registry.execute(&mut document, "Layer New Empty").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry
        .execute(&mut document, "Layer Delete Empty")
        .unwrap();
    assert!(document.layer_by_name("Empty").is_none());
    registry.execute(&mut document, "Undo").unwrap();
    assert!(document.layer_by_name("Empty").is_some());
}

#[test]
fn group_and_ungroup_all_visible_unlocked_objects_atomically() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0,0").unwrap();
    registry.execute(&mut document, "Point 1,0,0").unwrap();
    registry
        .execute(&mut document, "Layer New Hidden Geometry")
        .unwrap();
    registry.execute(&mut document, "Point 2,0,0").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry
        .execute(&mut document, "Layer Hide Hidden Geometry")
        .unwrap();
    registry
        .execute(&mut document, "Layer New Locked Geometry")
        .unwrap();
    registry.execute(&mut document, "Point 3,0,0").unwrap();
    registry
        .execute(&mut document, "Layer Current Default")
        .unwrap();
    registry
        .execute(&mut document, "Layer Lock Locked Geometry")
        .unwrap();

    registry.execute(&mut document, "Group All Pair").unwrap();
    let group = document.group_by_name("Pair").unwrap();
    assert_eq!(group.members().len(), 2);
    assert!(document.group_by_name("pair").is_none());
    registry.execute(&mut document, "Group All PAIR").unwrap();
    assert_eq!(document.groups().len(), 2);

    registry.execute(&mut document, "Ungroup Pair").unwrap();
    assert_eq!(document.groups().len(), 1);
    assert!(document.group_by_name("PAIR").is_some());
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.group_by_name("Pair").unwrap().members().len(), 2);
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.groups().len(), 1);
}

#[test]
fn set_object_name_matches_rhino_counter_order_and_preserves_selection_groups() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0").unwrap();
    registry.execute(&mut document, "Point 1,0").unwrap();
    registry.execute(&mut document, "Point 2,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group").unwrap();
    assert_eq!(
        document.group_by_name("Group01").unwrap().members().len(),
        3
    );
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();

    assert_eq!(
        registry
            .execute(
                &mut document,
                "SetObjectName _AppendCounter=_Yes \"Fastener Part\"",
            )
            .unwrap(),
        "Named 3 object(s)"
    );
    assert_eq!(document.undo_label(), Some("SetObjectName"));
    assert_eq!(
        ids.iter()
            .map(|id| document.object(*id).unwrap().attributes().name())
            .collect::<Vec<_>>(),
        vec![
            Some("Fastener Part 0"),
            Some("Fastener Part 1"),
            Some("Fastener Part 2")
        ]
    );
    assert_eq!(document.selected_object_count(), 3);
    assert_eq!(
        document.group_by_name("Group01").unwrap().members().len(),
        3
    );

    registry.execute(&mut document, "Undo").unwrap();
    assert!(
        document
            .objects()
            .all(|object| object.attributes().name().is_none())
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "SetObjectName Shared Name")
            .unwrap(),
        "Named 3 object(s)"
    );
    assert!(
        document
            .objects()
            .all(|object| object.attributes().name() == Some("Shared Name"))
    );
    assert_eq!(
        registry
            .execute(&mut document, "SetObjectName \"\"")
            .unwrap(),
        "Cleared names on 3 object(s)"
    );
    assert!(
        document
            .objects()
            .all(|object| object.attributes().name().is_none())
    );

    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "SetObjectName AppendCounter=Maybe Part"),
        Err(CommandError::Usage(SET_OBJECT_NAME_USAGE))
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), history.as_deref());
    document.clear_selection();
    assert!(matches!(
        registry.execute(&mut document, "SetObjectName Part"),
        Err(CommandError::NoObjectsSelected)
    ));
}

#[test]
fn set_object_color_preserves_identity_groups_selection_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Point 0,0").unwrap();
    registry.execute(&mut document, "Point 1,0").unwrap();
    registry.execute(&mut document, "SelAll").unwrap();
    registry.execute(&mut document, "Group Colors").unwrap();
    let ids = document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();

    assert_eq!(
        registry
            .execute(&mut document, "_SetObjectColor 12,34,56")
            .unwrap(),
        "Set 2 object color(s) to 12,34,56"
    );
    assert_eq!(document.undo_label(), Some("SetObjectColor"));
    for id in &ids {
        let attributes = document.object(*id).unwrap().attributes();
        assert_eq!(attributes.color_source(), ObjectColorSource::Object);
        assert_eq!(attributes.object_color(), ColorRgb::new(12, 34, 56));
    }
    assert_eq!(document.selected_object_count(), 2);
    assert_eq!(document.group_by_name("Colors").unwrap().members().len(), 2);

    registry.execute(&mut document, "Undo").unwrap();
    assert!(
        document
            .objects()
            .all(|object| { object.attributes().color_source() == ObjectColorSource::Layer })
    );
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(
        registry
            .execute(&mut document, "SetObjectColor _ByLayer")
            .unwrap(),
        "Set 2 object color(s) to ByLayer"
    );
    for object in document.objects() {
        assert_eq!(object.attributes().color_source(), ObjectColorSource::Layer);
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(12, 34, 56)
        );
    }

    let before = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    assert!(matches!(
        registry.execute(&mut document, "SetObjectColor 256,0,0"),
        Err(CommandError::InvalidColor(_))
    ));
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(document.undo_label(), history.as_deref());
}
