use super::*;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn points_are_live_and_session_undo_does_not_touch_prior_geometry() {
    let mut app = test_app();
    enter(&mut app, "Point 99,0,0");
    let prior = app.document.objects().next().unwrap().id();
    enter(&mut app, "Points");
    enter(&mut app, "1,2,3");
    assert_eq!(app.document.objects().len(), 2);
    enter(&mut app, "1,2,3");
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.last_point, Some(point(1., 2., 3.)));
    enter(&mut app, "7,8,9");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(app.points_session.is_none());
    assert_eq!(app.document.undo_label(), Some("Points"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.document.object(prior).is_some());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 3);
}

#[test]
fn escape_and_starting_another_command_keep_accepted_points() {
    for escape in [true, false] {
        let mut app = test_app();
        enter(&mut app, "Points");
        assert!(app.accept_drafting_point(point(1., 2., 3.)));
        if escape {
            app.cancel_interactive_command(true);
        } else {
            enter(&mut app, "Line");
        }
        assert_eq!(app.document.objects().len(), 1);
        assert!(app.points_session.is_none());
        app.cancel_interactive_command(false);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 0);
    }
}

#[test]
fn undoing_all_points_restores_prior_redo_and_relative_input_base() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    enter(&mut app, "Undo");
    app.last_point = Some(point(5., 6., 7.));
    enter(&mut app, "Points");
    enter(&mut app, "Undo");
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "1,2,3");
    enter(&mut app, "Undo");
    assert_eq!(app.last_point, Some(point(5., 6., 7.)));
    app.cancel_interactive_command(true);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn invalid_coordinate_keeps_the_session_and_its_live_points() {
    let mut app = test_app();
    enter(&mut app, "Points");
    enter(&mut app, "1,2,3");
    enter(&mut app, "4,5,NaN");
    assert_eq!(app.active_command, Some(InteractiveCommand::Points));
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "");
    assert!(app.points_session.is_none());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn sidebar_edits_finish_points_and_have_separate_history() {
    let mut app = test_app();
    enter(&mut app, "Points");
    enter(&mut app, "1,2,3");
    app.apply_sidebar_action(SidebarAction::AddLayer {
        name: "Next".into(),
    });
    assert!(app.points_session.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.layers().len(), 2);
    assert_eq!(app.document.undo_label(), Some("Add layer"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.layers().len(), 1);
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
}
