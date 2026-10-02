use super::*;

#[test]
fn repeated_transform_steps_share_one_history_entry_and_failed_steps_preserve_preferences() {
    let registry = CommandRegistry::with_builtins();
    let mut document = source();
    let before = document.objects().cloned().collect::<Vec<_>>();
    registry
        .execute(&mut document, "RememberCopyOptions No")
        .unwrap();
    registry.begin_copy_options("Scale");
    let mut group = document.begin_history_group("Scale").unwrap();
    for input in ["Scale 0,0,0 2 Copy=Yes", "Scale 0,0,0 3 Copy=Yes"] {
        registry
            .execute_in_history_group(&mut document, input, CommandContext::default(), &mut group)
            .unwrap();
    }
    let after = document.objects().cloned().collect::<Vec<_>>();
    assert_eq!(after.len(), 3);
    assert!(
        registry
            .execute_in_history_group(
                &mut document,
                "Scale 0,0,0 0 Copy=No",
                CommandContext::default(),
                &mut group
            )
            .is_err()
    );
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    assert!(document.history_group_is_current(&group));
    registry
        .execute(&mut document, "RememberCopyOptions Yes")
        .unwrap();
    assert_eq!(registry.copy_default("Scale"), Some(true));
    registry.execute(&mut document, "Undo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!document.can_undo());
    assert!(
        registry
            .execute_in_history_group(
                &mut document,
                "Scale 0,0,0 4 Copy=Yes",
                CommandContext::default(),
                &mut group
            )
            .is_err()
    );
    assert_eq!(document.redo_label(), Some("Scale"));
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
}

fn source() -> Document {
    let mut document = Document::default();
    let id = document
        .add_geometry(Geometry::Point(Point3::try_new(2., 3., 4.).unwrap()))
        .unwrap();
    document.select_object(id, SelectionMode::Replace).unwrap();
    document.clear_history().unwrap();
    document
}

#[test]
fn completed_choices_follow_aliases_across_documents_and_are_independent() {
    let registry = CommandRegistry::with_builtins();
    let mut first = source();
    registry
        .execute(&mut first, "_Rotate 0,0,0 90 _Copy=_Yes")
        .unwrap();
    let mut second = source();
    registry.execute(&mut second, "-rOtAtE 0,0,0 90").unwrap();
    assert_eq!(second.objects().len(), 2);
    assert_eq!(registry.copy_default("Rotate"), Some(true));
    assert_eq!(registry.copy_default("Scale"), Some(false));
    assert_eq!(registry.copy_default("Mirror"), Some(true));
    assert_eq!(registry.copy_default("Move"), None);
    assert_eq!(registry.copy_default("Missing"), None);
    assert_eq!(
        CommandRegistry::with_builtins().copy_default("Rotate"),
        Some(false)
    );

    registry.complete_copy_options("OrientOnSurface", false);
    assert_eq!(registry.copy_default("_OrientOnSrf"), Some(false));
}

#[test]
fn disabled_start_resets_only_the_started_command_and_survives_cancellation() {
    let registry = CommandRegistry::with_builtins();
    let mut document = source();
    registry.complete_copy_options("Rotate", true);
    registry.complete_copy_options("Scale", true);
    registry
        .execute(&mut document, "RememberCopyOptions No")
        .unwrap();
    assert_eq!(registry.copy_default("Rotate"), Some(false));
    assert_eq!(registry.begin_copy_options("Rotate"), Some(false));
    // No completion: any transient Copy=Yes choice is discarded.
    registry
        .execute(&mut document, "RememberCopyOptions Yes")
        .unwrap();
    assert_eq!(registry.copy_default("Rotate"), Some(false));
    assert_eq!(registry.copy_default("Scale"), Some(true));

    registry
        .execute(&mut document, "RememberCopyOptions No")
        .unwrap();
    registry
        .execute(&mut document, "Rotate 0,0,0 90 Copy=Yes")
        .unwrap();
    registry
        .execute(&mut document, "RememberCopyOptions Yes")
        .unwrap();
    assert_eq!(registry.copy_default("Rotate"), Some(true));
    registry
        .execute(&mut document, "RememberCopyOptions No")
        .unwrap();
    assert!(
        registry
            .execute(&mut document, "Rotate 0,0,0 bad Copy=Yes")
            .is_err()
    );
    registry
        .execute(&mut document, "RememberCopyOptions Yes")
        .unwrap();
    assert_eq!(registry.copy_default("Rotate"), Some(false));
}

#[test]
fn settings_and_queries_do_not_touch_geometry_selection_history_or_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut document = source();
    registry
        .execute(&mut document, "Rotate 0,0,0 90 Copy=Yes")
        .unwrap();
    let after = document.objects().cloned().collect::<Vec<_>>();
    registry.execute(&mut document, "Undo").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    let selected = document.selected_object_ids().collect::<Vec<_>>();
    assert_eq!(registry.copy_default("Rotate"), Some(true));
    for input in [
        "RememberCopyOptions",
        "_RememberCopyOptions _No",
        "RememberCopyOptions Yes",
    ] {
        registry.execute(&mut document, input).unwrap();
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), selected);
        assert!(!document.can_undo());
        assert_eq!(document.redo_label(), Some("Rotate"));
    }
    for input in ["RememberCopyOptions Maybe", "RememberCopyOptions Yes No"] {
        assert!(registry.execute(&mut document, input).is_err());
        assert!(registry.remember_copy_options());
        assert_eq!(document.redo_label(), Some("Rotate"));
    }
    registry.execute(&mut document, "Redo").unwrap();
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), after);
    assert_eq!(registry.copy_default("Rotate"), Some(true));
}

#[test]
fn invalid_copy_edits_never_replace_a_completed_choice() {
    let registry = CommandRegistry::with_builtins();
    let mut document = source();
    registry
        .execute(&mut document, "Scale 0,0,0 2 Copy=Yes")
        .unwrap();
    registry.execute(&mut document, "Undo").unwrap();
    let before = document.objects().cloned().collect::<Vec<_>>();
    for input in [
        "Scale 0,0,0 2 Copy=Maybe",
        "Scale 0,0,0 2 Copy=No Copy=Yes",
        "Scale 0,0,0 0 Copy=No",
        "Scale 0,0,0 2 Copy",
    ] {
        assert!(registry.execute(&mut document, input).is_err(), "{input}");
        assert_eq!(registry.copy_default("Scale"), Some(true));
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.redo_label(), Some("Scale"));
    }
    document.clear_selection();
    assert!(
        registry
            .execute(&mut document, "Scale 0,0,0 2 Copy=No")
            .is_err()
    );
    assert_eq!(registry.copy_default("Scale"), Some(true));
}

#[test]
fn shared_policy_covers_each_implemented_affected_command() {
    let registry = CommandRegistry::with_builtins();
    let names = [
        "ExtractSrf",
        "ExtractSubCrv",
        "RemoveFromGroup",
        "Orient",
        "Orient3Pt",
        "OrientOnSrf",
        "Scale",
        "Scale1D",
        "Scale2D",
        "ScaleNU",
        "Rotate",
        "Rotate3D",
        "Mirror",
        "Shear",
        "SetPt",
    ];
    for name in names {
        let default = matches!(name, "Mirror" | "OrientOnSrf");
        assert_eq!(registry.copy_default(name), Some(default), "{name}");
        registry.complete_copy_options(name, !default);
        assert_eq!(registry.begin_copy_options(name), Some(!default), "{name}");
    }
    registry
        .execute(&mut source(), "RememberCopyOptions No")
        .unwrap();
    for name in names {
        assert_eq!(
            registry.begin_copy_options(name),
            Some(matches!(name, "Mirror" | "OrientOnSrf")),
            "{name}"
        );
    }
    registry
        .execute(&mut source(), "RememberCopyOptions Yes")
        .unwrap();
    for name in names {
        assert_eq!(
            registry.copy_default(name),
            Some(matches!(name, "Mirror" | "OrientOnSrf")),
            "{name}"
        );
    }
}
