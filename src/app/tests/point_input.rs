use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn typed_points_continue_line_instead_of_cancelling_it() {
    let mut app = test_app();
    for input in ["Line", "1,2,3", "4,6,3"] {
        enter(&mut app, input);
    }
    let Geometry::Line(line) = app
        .document
        .objects()
        .next()
        .expect("created line")
        .geometry()
    else {
        panic!("line");
    };
    assert_eq!(line.start(), point(1.0, 2.0, 3.0));
    assert_eq!(line.end(), point(4.0, 6.0, 3.0));
    assert!(app.active_command.is_none());
    assert_eq!(app.document.undo_label(), Some("Line"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
}

#[test]
fn typed_points_use_each_active_construction_plane_with_world_override() {
    for (viewport, expected) in [
        (0, [1.0, 2.0, 3.0]),
        (1, [1.0, 2.0, 3.0]),
        (2, [1.0, -3.0, 2.0]),
        (3, [3.0, 1.0, 2.0]),
    ] {
        let mut app = test_app();
        app.active_viewport = viewport;
        for input in ["Line", "1,2,3", "wr2,3,4"] {
            enter(&mut app, input);
        }
        let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
            panic!("line");
        };
        assert_eq!(line.start().to_array(), expected);
        assert_eq!(
            line.end().to_array(),
            [expected[0] + 2.0, expected[1] + 3.0, expected[2] + 4.0]
        );
    }
}

#[test]
fn mouse_and_typed_relative_points_share_a_polyline_without_grid_rounding() {
    let mut app = test_app();
    enter(&mut app, "Polyline");
    assert!(app.accept_drafting_point(point(1.25, 2.5, 0.75)));
    for input in ["r2,3", "@2<90,1", ""] {
        enter(&mut app, input);
    }
    let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
        panic!("polyline");
    };
    assert_eq!(
        polyline.vertices(),
        &[
            point(1.25, 2.5, 0.75),
            point(3.25, 5.5, 0.75),
            point(3.25, 7.5, 1.75)
        ]
    );
    assert!(app.osnap && app.smart_track && app.grid_snap);
    assert_eq!(app.document.undo_label(), Some("Polyline"));
}

#[test]
fn invalid_typed_points_preserve_the_draft_last_point_and_editable_input() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "r1,2");
    assert_eq!(app.last_point, None);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Line { start: None })
    );
    assert_eq!(app.command_input, "r1,2");
    enter(&mut app, "1,2,3");
    let active = app.active_command;
    for input in [
        "1,,2",
        "NaN,0",
        "1e309,0",
        "wInf,0",
        "5",
        "1, 2",
        "w 1,2",
        "bad,1",
        "@",
        "rw",
        "w5<30<120",
    ] {
        enter(&mut app, input);
        assert_eq!(app.active_command, active);
        assert_eq!(app.last_point, Some(point(1.0, 2.0, 3.0)));
        assert_eq!(app.command_input, input);
        assert!(!app.document.can_undo());
    }
    enter(&mut app, "r1,0");
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.command_input.is_empty());
}

#[test]
fn rejected_geometric_pick_does_not_replace_the_relative_origin() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "1,2,3");
    enter(&mut app, "1,2,3"); // A coincident radius point is rejected.
    assert_eq!(app.last_point, Some(point(1.0, 2.0, 3.0)));
    assert!(app.active_command.is_some());
    enter(&mut app, "r2,0");
    let Geometry::Circle(circle) = app.document.objects().next().unwrap().geometry() else {
        panic!("circle");
    };
    assert_eq!(circle.center(), point(1.0, 2.0, 3.0));
    assert_eq!(circle.radius(), 2.0);
}

#[test]
fn command_names_aliases_and_script_prefixes_can_replace_a_point_prompt() {
    for replacement in ["Line", "L", "-Line", "_Line", "_-Line"] {
        let mut app = test_app();
        for input in ["Line", "1,2,3", replacement] {
            enter(&mut app, input);
        }
        assert_eq!(
            app.active_command,
            Some(InteractiveCommand::Line { start: None })
        );
        assert_eq!(app.document.objects().len(), 0);
    }
    let mut app = test_app();
    for input in ["Line", "1,2,3", "Point 8,9,10"] {
        enter(&mut app, input);
    }
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn cancellation_discards_geometry_but_remembers_accepted_interactive_points() {
    let mut app = test_app();
    for input in ["Polyline", "1,2,3", "r1,2", "1,,2"] {
        enter(&mut app, input);
    }
    app.cancel_interactive_command(true);
    assert!(app.command_input.is_empty());
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().len(), 0);
    assert!(!app.document.can_undo());
    for input in ["Point", "rw1,2,3"] {
        enter(&mut app, input);
    }
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(*p, point(3.0, 6.0, 6.0));
}

#[test]
fn typed_transform_points_use_the_existing_transactional_command() {
    let mut app = test_app();
    for input in ["Point 1,2,3", "SelAll", "Move", "0", "r4,5,6"] {
        enter(&mut app, input);
    }
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(*p, point(5.0, 7.0, 9.0));
    assert_eq!(app.document.undo_label(), Some("Move"));
    enter(&mut app, "Undo");
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(*p, point(1.0, 2.0, 3.0));
}

#[test]
fn typed_point_completion_retains_full_float_precision() {
    let mut app = test_app();
    enter(&mut app, "Point");
    enter(
        &mut app,
        "w1.1234567890123457,-2.123456789012345,0.000000123456789",
    );
    let Geometry::Point(p) = app.document.objects().next().unwrap().geometry() else {
        panic!("point");
    };
    assert_eq!(
        *p,
        point(1.1234567890123457, -2.123456789012345, 0.000000123456789)
    );
}

#[test]
fn a_mouse_pick_can_correct_invalid_typed_input_without_leaving_stale_text() {
    let mut app = test_app();
    for input in ["Line", "0", "1,,2"] {
        enter(&mut app, input);
    }
    app.handle_viewport_action(ViewportOutput {
        picked_point: Some(point(2.0, 3.0, 0.0)),
        ..ViewportOutput::default()
    });
    assert!(app.command_input.is_empty());
    assert!(app.active_command.is_none());
    assert_eq!(app.last_point, Some(point(2.0, 3.0, 0.0)));
    assert_eq!(app.document.objects().len(), 1);
}
