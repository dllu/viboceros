use super::*;

#[test]
fn distance_display_options_preserve_points_and_survive_local_undo() {
    let mut app = test_app();
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("Distance"));
    app.command_input = "Units=in".into();
    app.run_command();
    assert!(app.accept_drafting_point(point(9., 0., 0.)));
    app.execute_command("Undo");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Distance {
            start: None,
            display_units: Some("Inches"),
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    let state = app.active_command;
    app.command_input = "Units=invalid".into();
    app.run_command();
    assert_eq!(app.active_command, state);
    assert!(app.accept_drafting_point(point(25.4, 0., 0.)));
    let report = app.command_log.back().unwrap();
    assert!(report.ends_with("Inches"), "{report}");
    let value: f64 = report
        .lines()
        .last()
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    assert!((value - 1.).abs() < 1e-15, "{report}");
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn distance_can_recover_from_overflow_by_changing_display_units() {
    let mut app = test_app();
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("Distance"));
    assert!(app.accept_drafting_point(point(-1e308, 0., 0.)));
    assert!(!app.accept_drafting_point(point(1e308, 0., 0.)));
    let anchor = app.active_command.unwrap().anchor();
    app.execute_command("Units=km");
    assert_eq!(app.active_command.unwrap().anchor(), anchor);
    assert!(app.accept_drafting_point(point(1e308, 0., 0.)));
    assert!(app.command_log.back().unwrap().ends_with("Kilometres"));
    assert_eq!(format!("{:?}", app.document), before);

    assert!(app.try_start_interactive_command("Distance"));
    app.execute_command("Units=Inches");
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    app.execute_command("Units=Model_Units");
    assert!(app.accept_drafting_point(point(25.4, 0., 0.)));
    assert!(app.command_log.back().unwrap().ends_with("Distance = 25.4"));
    assert_eq!(format!("{:?}", app.document), before);
}

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
