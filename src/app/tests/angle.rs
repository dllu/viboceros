use super::*;

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
