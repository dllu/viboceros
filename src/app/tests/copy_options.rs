use super::*;
use crate::viewport::{ComponentClick, ComponentPick};
use viboceros_command::ComponentSelectionKind;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn remember_copy_prompt_accepts_answers_and_cancels_without_model_edits() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    enter(&mut app, "Rotate 0,0,0 90 Copy=Yes");
    enter(&mut app, "Undo");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let selection = app.document.selected_object_ids().collect::<Vec<_>>();
    enter(&mut app, "_RememberCopyOptions");
    assert!(app.remember_copy_prompt);
    assert!(!app.command_line_idle());
    enter(&mut app, "Maybe");
    assert!(app.remember_copy_prompt);
    assert!(app.commands.remember_copy_options());
    app.cancel_current_prompt_or_selection();
    assert!(!app.remember_copy_prompt);
    assert!(app.commands.remember_copy_options());
    enter(&mut app, "RememberCopyOptions");
    enter(&mut app, "_No");
    assert!(!app.commands.remember_copy_options());
    enter(&mut app, "RememberCopyOptions");
    enter(&mut app, "");
    assert!(!app.commands.remember_copy_options());
    enter(&mut app, "RememberCopyOptions");
    enter(&mut app, "_Cancel");
    assert!(!app.commands.remember_copy_options());
    enter(&mut app, "RememberCopyOptions");
    enter(&mut app, "_Yes");
    assert!(app.commands.remember_copy_options());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selection
    );
    assert_eq!(app.document.redo_label(), Some("Rotate"));
    assert_eq!(app.commands.copy_default("Rotate"), Some(true));
}

#[test]
fn extract_copy_defaults_are_saved_only_after_extraction_and_reset_at_disabled_start() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Brep(
            viboceros_geometry::Brep::try_box(
                viboceros_command::CommandContext::default().construction_plane,
                [[0., 2.], [0., 3.], [0., 4.]],
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "ExtractSrf Copy=Yes");
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: source,
            index: 0,
            kind: ComponentSelectionKind::BrepFace,
        }],
        preselection: false,
        modifiers: Default::default(),
    });
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.commands.copy_default("ExtractSrf"), Some(true));
    enter(&mut app, "Undo");
    enter(&mut app, "ExtractSrf");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtractSrf { copy: true, .. })
    ));
    enter(&mut app, "Copy=No");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.copy_default("ExtractSrf"), Some(true));
    assert_eq!(app.document.redo_label(), Some("ExtractSrf"));
    enter(&mut app, "RememberCopyOptions No");
    enter(&mut app, "ExtractSrf Copy=Yes");
    app.cancel_current_prompt_or_selection();
    enter(&mut app, "RememberCopyOptions Yes");
    enter(&mut app, "ExtractSrf");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::ExtractSrf { copy: false, .. })
    ));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.redo_label(), Some("ExtractSrf"));
}

#[test]
fn cancelled_group_selection_resets_copy_only_when_remembering_is_disabled() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    enter(&mut app, "Group");
    enter(&mut app, "RemoveFromGroup Copy=Yes");
    assert_eq!(app.commands.copy_default("RemoveFromGroup"), Some(true));
    enter(&mut app, "Undo");
    enter(&mut app, "SelNone");
    enter(&mut app, "RemoveFromGroup Copy=No");
    assert!(app.object_prompt.is_some());
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.copy_default("RemoveFromGroup"), Some(true));
    enter(&mut app, "RememberCopyOptions No");
    enter(&mut app, "RemoveFromGroup Copy=Yes");
    app.cancel_current_prompt_or_selection();
    enter(&mut app, "RememberCopyOptions Yes");
    assert_eq!(app.commands.copy_default("RemoveFromGroup"), Some(false));
    assert_eq!(app.document.redo_label(), Some("RemoveFromGroup"));
}

#[test]
fn set_point_uses_its_own_saved_copy_choice() {
    let mut app = test_app();
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "SelAll");
    enter(&mut app, "SetPt 5,6,7 Copy=Yes");
    enter(&mut app, "Undo");
    enter(&mut app, "SetPt");
    assert!(
        matches!(app.active_command, Some(InteractiveCommand::SetPoint { options }) if options.copy)
    );
    enter(&mut app, "Copy=No");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.copy_default("SetPt"), Some(true));
    assert_eq!(app.commands.copy_default("Rotate"), Some(false));
    enter(&mut app, "RememberCopyOptions No");
    enter(&mut app, "SetPt");
    assert!(
        matches!(app.active_command, Some(InteractiveCommand::SetPoint { options }) if !options.copy)
    );
    app.cancel_current_prompt_or_selection();
    enter(&mut app, "RememberCopyOptions Yes");
    assert_eq!(app.commands.copy_default("SetPt"), Some(false));
}
