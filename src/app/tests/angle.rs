use super::*;

#[test]
fn angle_two_objects_option_transitions_from_point_prompt_without_model_edits() {
    use viboceros_document::SelectionMode;
    for direct in [false, true] {
        let mut app = test_app();
        app.execute_command("Line 0,0,0 1,0,0");
        app.execute_command("Line 0,0,0 0,1,0");
        app.execute_command("Point 2,3,4");
        app.execute_command("Undo");
        let ids: Vec<_> = app.document.objects().map(|object| object.id()).collect();
        app.execute_command("SelNone");
        let before = format!("{:?}", app.document);
        let previous_last = app.last_point;
        assert!(app.try_start_interactive_command("Angle"));
        assert!(app.active_command.unwrap().prompt().contains("TwoObjects"));
        if direct {
            assert!(app.try_execute_command("_TwoObjects"));
        } else {
            app.command_input = "  _tWoObJeCtS  ".into();
            app.run_command();
        }
        assert!(app.active_command.is_none());
        assert!(app.object_prompt.is_some());
        assert!(app.command_input.is_empty());
        assert!(app.drafting_plane.is_none());
        assert_eq!(app.last_point, previous_last);
        assert_eq!(format!("{:?}", app.document), before);
        app.select_prompt_objects(ids, SelectionMode::Replace);
        let selected = format!("{:?}", app.document);
        assert!(app.try_continue_object_prompt(""));
        assert_eq!(app.command_log.back().unwrap(), "Angle = 90 degrees");
        assert_eq!(format!("{:?}", app.document), selected);
        app.execute_command("Redo");
        assert_eq!(app.document.objects().count(), 3);
    }
}

#[test]
fn angle_two_objects_transition_supports_new_preselection_and_cancellation() {
    use viboceros_document::SelectionMode;
    let mut app = test_app();
    app.execute_command("Line 0,0,0 1,0,0");
    app.execute_command("Line 0,0,0 0,1,0");
    app.execute_command("SelNone");
    let ids: Vec<_> = app.document.objects().map(|object| object.id()).collect();
    assert!(app.try_start_interactive_command("Angle"));
    // Selection may have changed since the initial point prompt opened.
    app.document
        .select_objects(ids, SelectionMode::Replace)
        .unwrap();
    let selected = format!("{:?}", app.document);
    assert!(app.try_continue_angle("TwoObjects"));
    assert!(app.active_command.is_none());
    assert!(app.object_prompt.is_none());
    assert_eq!(app.command_log.back().unwrap(), "Angle = 90 degrees");
    assert_eq!(format!("{:?}", app.document), selected);
    app.execute_command("SelNone");
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("Angle"));
    assert!(app.try_continue_angle("TwoObjects"));
    app.cancel_interactive_command(true);
    assert!(app.active_command.is_none());
    assert!(app.object_prompt.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn angle_late_two_objects_option_keeps_accepted_points_for_correction() {
    let mut app = test_app();
    assert!(!app.try_continue_angle("TwoObjects"));
    assert!(app.try_start_interactive_command("Angle"));
    assert!(!app.try_continue_angle("TwoObjects extra"));
    for p in [point(0., 0., 0.), point(1., 0., 0.), point(0., 0., 0.)] {
        assert!(app.accept_drafting_point(p));
        let pending = app.active_command;
        let before = format!("{:?}", app.document);
        app.command_input = "TwoObjects".into();
        app.run_command();
        assert_eq!(app.active_command, pending);
        assert!(app.object_prompt.is_none());
        assert_eq!(app.last_point, Some(p));
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.command_log.back().unwrap().starts_with("Error:"));
    }
    assert!(app.accept_drafting_point(point(0., 1., 0.)));
    assert_eq!(app.command_log.back().unwrap(), "Angle = 90 degrees");
}

#[test]
fn angle_point_picking_accepts_finite_points_with_overflowing_differences() {
    let mut app = test_app();
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("Angle"));
    for p in [
        point(-1e308, 0., 0.),
        point(1e308, 0., 0.),
        point(0., -1e308, 0.),
        point(0., 1e308, 0.),
    ] {
        assert!(app.accept_drafting_point(p));
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.command_log.back().unwrap(), "Angle = 90 degrees");
    assert_eq!(format!("{:?}", app.document), before);
}

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
