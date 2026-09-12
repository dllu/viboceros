use super::*;

#[test]
fn distance_undo_through_direct_dispatch_restores_an_absent_anchor() {
    let mut app = test_app();
    app.last_point = None;
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("Distance"));
    assert!(app.accept_drafting_point(point(1., 2., 3.)));
    app.execute_command("Undo");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Distance { start: None, .. })
    ));
    assert_eq!(app.last_point, None);
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_interactive_command(false);
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn distance_undo_revises_input_without_replaying_document_history() {
    let mut app = test_app();
    app.execute_command("Point 9,8,7");
    app.execute_command("Point 6,5,4");
    app.execute_command("Undo");
    app.last_point = Some(point(1., 2., 3.));
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("Distance"));
    assert!(app.accept_drafting_point(point(20., 30., 40.)));
    app.command_input = "_uNdO".into();
    app.run_command();
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Distance { start: None, .. })
    ));
    assert_eq!(app.last_point, Some(point(1., 2., 3.)));
    assert!(app.drafting_plane.is_none());
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.try_continue_point_input("r1,0,0"));
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(2., 2., 3.))
    );
    assert!(app.accept_drafting_point(point(5., 6., 3.)));
    assert!(app.command_log.back().unwrap().ends_with("Distance = 5"));
    assert_eq!(format!("{:?}", app.document), before);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 2);
}
