use super::*;

#[test]
fn angle_object_mode_supports_postselection_and_bare_preselection() {
    use viboceros_document::SelectionMode;
    let mut app = test_app();
    app.execute_command("Line 0,0,0 1,0,0");
    app.execute_command("Line 4,5,6 3,6,6");
    let ids: Vec<_> = app.document.objects().map(|object| object.id()).collect();
    app.execute_command("SelNone");
    app.command_input = "Angle TwoObjects".into();
    app.run_command();
    assert!(app.object_prompt.is_some());
    assert!(app.active_command.is_none());
    app.select_prompt_objects([ids[0]], SelectionMode::Replace);
    let incomplete = format!("{:?}", app.document);
    assert!(app.try_continue_object_prompt(""));
    assert!(app.object_prompt.is_some());
    assert_eq!(format!("{:?}", app.document), incomplete);
    app.select_prompt_objects([ids[1]], SelectionMode::Add);
    let before = format!("{:?}", app.document);
    assert!(app.try_continue_object_prompt(""));
    assert!(app.object_prompt.is_none());
    assert!(app.command_log.back().unwrap().starts_with("Angle ="));
    assert_eq!(format!("{:?}", app.document), before);
    app.command_input = "Angle".into();
    app.run_command();
    assert!(app.active_command.is_none());
    assert!(app.object_prompt.is_none());
    assert!(app.command_log.back().unwrap().starts_with("Angle ="));
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn angle_point_picking_rejects_zero_directions_without_model_history_changes() {
    let mut app = test_app();
    app.execute_command("Point 1,2,3");
    app.execute_command("Point 4,5,6");
    app.execute_command("Undo");
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("_Angle"));
    assert!(app.try_continue_point_input("w0,0,0"));
    let first = app.active_command;
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert_eq!(app.active_command, first);
    assert!(app.accept_drafting_point(point(1., 0., 0.)));
    assert_eq!(app.active_command.unwrap().anchor(), None);
    assert!(app.accept_drafting_point(point(4., 5., 6.)));
    let second = app.active_command;
    assert!(!app.accept_drafting_point(point(4., 5., 6.)));
    assert_eq!(app.active_command, second);
    assert!(app.accept_drafting_point(point(4., 6., 6.)));
    assert_eq!(app.active_command, None);
    assert_eq!(app.command_log.back().unwrap(), "Angle = 90 degrees");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.try_start_interactive_command("Angle"));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    app.cancel_interactive_command(false);
    assert_eq!(format!("{:?}", app.document), before);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 2);
}
