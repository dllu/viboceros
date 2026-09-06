use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}
fn setup() -> VibocerosApp {
    let mut app = test_app();
    for command in ["Point 0,0,0", "Point 3,0,0", "Point 10,0,0", "SelAll"] {
        enter(&mut app, command);
    }
    app
}

#[test]
fn bare_distribution_collects_points_with_correction_and_one_undo_step() {
    let mut app = setup();
    enter(&mut app, "Distribute Mode=Center");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Distribute { start: None, .. })
    ));
    enter(&mut app, "w0,0,0");
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "w0,0,0");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Distribute { start: Some(_), .. })
    ));
    assert_eq!(app.document.undo_label(), history.as_deref());
    enter(&mut app, "w1,0,0");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document
            .object(app.document.selected_object_ids().nth(1).unwrap())
            .unwrap()
            .geometry(),
        &Geometry::Point(point(5., 0., 0.))
    );
    assert_eq!(app.document.undo_label(), Some("Distribute"));
    enter(&mut app, "Undo");
    assert_eq!(
        app.document.objects().nth(1).unwrap().geometry(),
        &Geometry::Point(point(3., 0., 0.))
    );
    enter(&mut app, "Redo");
    assert_eq!(
        app.document.objects().nth(1).unwrap().geometry(),
        &Geometry::Point(point(5., 0., 0.))
    );
}

#[test]
fn picked_and_typed_spatial_directions_share_settings_and_geometry() {
    let mut typed = setup();
    let mut picked = setup();
    enter(&mut typed, "Distribute Direction Mode=Center Spacing=2");
    enter(&mut typed, "w1,2,3");
    enter(&mut typed, "w3,5,10");
    assert!(picked.try_start_interactive_command("Distribute Direction Mode=Center Spacing=2"));
    assert!(picked.accept_drafting_point(point(1., 2., 3.)));
    assert!(picked.accept_drafting_point(point(3., 5., 10.)));
    assert!(typed.active_command.is_none() && picked.active_command.is_none());
    assert_eq!(
        typed
            .document
            .objects()
            .map(|o| o.geometry())
            .collect::<Vec<_>>(),
        picked
            .document
            .objects()
            .map(|o| o.geometry())
            .collect::<Vec<_>>()
    );
}

#[test]
fn explicit_axis_dispatch_and_cancellation_do_not_confuse_the_point_prompt() {
    let mut app = setup();
    enter(&mut app, "Distribute XAxis Mode=Center Spacing=3");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.objects().nth(2).unwrap().geometry(),
        &Geometry::Point(point(6., 0., 0.))
    );
    enter(&mut app, "Distribute Direction");
    enter(&mut app, "w0,0,0");
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(!app.try_start_interactive_command("Distribute Direction Mode=Wrong"));
    assert!(!app.try_start_interactive_command("Distribute Direction Spacing=NaN"));
}

#[test]
fn insufficient_groups_are_rejected_before_collecting_direction_points() {
    let mut app = setup();
    enter(&mut app, "Group Together");
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    let history = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "Distribute");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert_eq!(app.document.undo_label(), history.as_deref());
    assert!(
        app.command_log
            .iter()
            .any(|line| line.contains("at least three objects or groups"))
    );
}
