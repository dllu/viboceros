use super::*;

#[test]
fn evaluate_point_uses_pick_view_plane_and_preserves_selection_and_history() {
    let mut app = test_app();
    app.execute_command("Point 1,2,3");
    app.execute_command("SelAll");
    app.execute_command("Point 4,5,6");
    app.execute_command("Undo");
    let before = format!("{:?}", app.document);
    let frame = Frame3::try_from_directions(
        point(10., 20., 30.),
        viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[app.active_viewport].plane.set(frame);
    for input in ["EvaluatePt", "_EvaluatePt _Label=_No"] {
        assert!(app.try_start_interactive_command(input));
        assert!(app.try_continue_point_input("4,5,3"));
        assert!(app.active_command.is_none());
        assert_eq!(
            app.command_log.back().unwrap(),
            "World coordinates = 13,24,35\nCPlane coordinates = 4,5,3"
        );
        assert_eq!(format!("{:?}", app.document), before);
    }
    assert!(app.try_start_interactive_command("EvaluatePt"));
    app.cancel_interactive_command(true);
    assert_eq!(format!("{:?}", app.document), before);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.try_start_interactive_command("EvaluatePt Label=Yes"));
    assert!(!app.try_start_interactive_command("EvaluatePt 1,2,3"));
}

#[test]
fn evaluate_point_retains_prompt_after_unrepresentable_local_coordinate() {
    let mut app = test_app();
    let plane = app.viewports[app.active_viewport]
        .construction_plane()
        .with_origin(point(-1e308, 0., 0.));
    app.viewports[app.active_viewport].plane.set(plane);
    let before = format!("{:?}", app.document);
    let previous_last = app.last_point;
    assert!(app.try_start_interactive_command("EvaluatePt"));
    assert!(!app.accept_drafting_point(point(1e308, 0., 0.)));
    assert_eq!(app.active_command, Some(InteractiveCommand::EvaluatePoint));
    assert_eq!(app.last_point, previous_last);
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.active_command.is_none());
    assert!(app.command_log.back().unwrap().contains("1e308"));
}
