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

#[test]
fn transparent_plane_enter_does_not_finish_live_points() {
    for plane_command in [
        "CPlane",
        "CPlane 3Point",
        "CPlane Elevation",
        "CPlane Rotate",
    ] {
        let mut app = test_app();
        enter(&mut app, "Points");
        enter(&mut app, "1,2,3");
        enter(&mut app, plane_command);
        assert!(app.plane_prompt.is_some());
        enter(&mut app, "");
        assert_eq!(app.active_command, Some(InteractiveCommand::Points));
        assert!(app.points_session.is_some());
        assert_eq!(app.document.objects().len(), 1);
        if plane_command == "CPlane" {
            assert!(app.plane_prompt.is_none());
        } else {
            assert_eq!(
                app.plane_prompt.as_ref().unwrap().points.len(),
                usize::from(plane_command == "CPlane 3Point")
            );
            app.cancel_plane_prompt();
        }
        enter(&mut app, "");
        assert!(app.points_session.is_none());
        assert_eq!(app.document.undo_label(), Some("Points"));
    }
}

#[test]
fn modeling_undo_exits_transparent_plane_prompt_before_removing_session_point() {
    let mut app = test_app();
    enter(&mut app, "Points");
    enter(&mut app, "1,2,3");
    enter(&mut app, "CPlane 3Point");
    enter(&mut app, "Undo");
    assert!(app.plane_prompt.is_none());
    assert_eq!(app.active_command, Some(InteractiveCommand::Points));
    assert!(app.points_session.is_some());
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "");
    assert!(app.points_session.is_none());
}

#[test]
fn completed_transparent_plane_and_interface_edits_preserve_live_point_history() {
    let mut app = test_app();
    enter(&mut app, "Points");
    enter(&mut app, "w1,2,3");
    let last = app.last_point;
    for input in [
        "CPlane 3Point",
        "w10,20,30",
        "w10,21,30",
        "Snap",
        "w10,20,31",
    ] {
        enter(&mut app, input);
    }
    assert!(app.plane_prompt.is_none());
    assert_eq!(app.last_point, last);
    assert_eq!(app.active_command, Some(InteractiveCommand::Points));
    enter(&mut app, "2,3");
    enter(&mut app, "");
    let expected = [point(1., 2., 3.), point(10., 22., 33.)];
    let actual = app
        .document
        .objects()
        .map(|o| {
            if let Geometry::Point(p) = o.geometry() {
                *p
            } else {
                panic!()
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    let frame = app.viewports[0].construction_plane();
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.viewports[0].construction_plane(), frame);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn points_handler_declines_input_owned_by_a_transparent_plane_prompt() {
    let mut app = test_app();
    enter(&mut app, "Points");
    enter(&mut app, "1,2,3");
    enter(&mut app, "CPlane 3Point");
    assert!(!app.try_continue_points(""));
    assert!(!app.try_continue_points("Undo"));
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.points_session.is_some());
    app.cancel_plane_prompt();
    app.cancel_interactive_command(false);
    assert_eq!(app.document.undo_label(), Some("Points"));
}
