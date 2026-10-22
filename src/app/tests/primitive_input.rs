use super::*;

#[test]
fn interactive_line_uses_the_transactional_command_path() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("_Line"));
    app.accept_drafting_point(point(1.0, 2.0, 3.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Line {
            start: Some(point(1.0, 2.0, 3.0))
        })
    );

    app.accept_drafting_point(point(4.0, 6.0, 3.0));
    assert_eq!(app.active_command, None);
    let Geometry::Line(line) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created line")
    };
    assert_eq!(line.start(), point(1.0, 2.0, 3.0));
    assert_eq!(line.end(), point(4.0, 6.0, 3.0));
    assert_eq!(app.document.undo_label(), Some("Line"));
}

#[test]
fn interactive_distance_accepts_typed_and_picked_points_without_model_edits() {
    let mut app = test_app();
    let before = format!("{:?}", app.document);
    assert!(app.try_start_interactive_command("_Distance"));
    assert!(app.try_continue_point_input("w1,2,3"));
    assert_eq!(
        app.active_command.unwrap().anchor(),
        Some(point(1., 2., 3.))
    );
    assert!(app.accept_drafting_point(point(4., 6., 3.)));
    assert_eq!(app.active_command, None);
    assert!(app.command_log.back().unwrap().ends_with("Distance = 5"));
    assert_eq!(format!("{:?}", app.document), before);

    assert!(app.try_start_interactive_command("Distance"));
    assert!(app.accept_drafting_point(point(1., 2., 3.)));
    assert!(app.accept_drafting_point(point(1., 2., 3.)));
    assert!(app.command_log.back().unwrap().ends_with("Distance = 0"));
    assert_eq!(format!("{:?}", app.document), before);

    assert!(app.try_start_interactive_command("Distance"));
    assert!(app.accept_drafting_point(point(1., 2., 3.)));
    app.cancel_interactive_command(true);
    assert_eq!(app.active_command, None);
    assert_eq!(format!("{:?}", app.document), before);

    assert!(app.try_start_interactive_command("Distance"));
    assert!(app.accept_drafting_point(point(-1e308, 0., 0.)));
    assert!(!app.accept_drafting_point(point(1e308, 0., 0.)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Distance { start: Some(_), .. })
    ));
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn interactive_circle_size_options_accept_numbers_and_picks() {
    let mut app = test_app();
    let center = point(0.0, 0.0, 0.0);
    let sizes = [
        ("Diameter", "6", 3.0),
        ("Circumference", "18.84955592153876", 3.0),
        ("Area", "28.274333882308138", 3.0),
    ];
    for (mode, value, expected_radius) in sizes {
        assert!(app.try_start_interactive_command("Circle"));
        assert!(app.accept_drafting_point(center));
        assert!(app.try_continue_circle(mode));
        assert!(app.try_continue_circle(value));
        assert_eq!(app.active_command, None);
        assert!(matches!(app.document.objects().last().unwrap().geometry(),
            Geometry::Circle(circle) if (circle.radius() - expected_radius).abs() < 1e-12));
    }

    for (mode, expected_radius) in [
        ("Diameter", 6.0),
        ("Circumference", 6.0 / std::f64::consts::TAU),
        ("Area", (6.0 / std::f64::consts::PI).sqrt()),
    ] {
        assert!(app.try_start_interactive_command("Circle"));
        assert!(app.accept_drafting_point(center));
        assert!(app.try_continue_circle(mode));
        assert!(app.accept_drafting_point(point(6.0, 0.0, 0.0)));
        assert_eq!(app.active_command, None);
        assert!(matches!(app.document.objects().last().unwrap().geometry(),
            Geometry::Circle(circle) if (circle.radius() - expected_radius).abs() < 1e-12));
    }

    assert!(app.try_start_interactive_command("Circle"));
    assert!(app.accept_drafting_point(center));
    assert!(app.try_continue_circle("Area"));
    let before = format!("{:?}", app.document);
    assert!(app.try_continue_circle("-1"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::CircleSize {
            center,
            mode: CircleSizeMode::Area
        })
    );
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.try_continue_circle("9"));
    assert_eq!(app.active_command, None);
}

#[test]
fn a_degenerate_second_pick_keeps_line_active() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("L"));
    let start = point(2.0, 3.0, 0.0);
    app.accept_drafting_point(start);
    app.accept_drafting_point(start);

    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Line { start: Some(start) })
    );
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.command_log.back().unwrap().contains("line end"));
}

#[test]
fn interactive_circle_and_arc_reject_degenerate_picks_and_use_history() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Circle"));
    let center = point(0.0, 0.0, 2.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(center);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Circle {
            center: Some(center)
        })
    );
    app.accept_drafting_point(point(2.0, 0.0, 2.0));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Circle(circle) if circle.center() == center && circle.radius() == 2.0
    ));
    assert_eq!(app.document.undo_label(), Some("Circle"));

    assert!(app.try_start_interactive_command("Arc StartPoint ThroughPoint"));
    app.accept_drafting_point(point(5.0, 0.0, 0.0));
    app.accept_drafting_point(point(6.0, 0.0, 0.0));
    app.accept_drafting_point(point(7.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Arc {
            points: [Some(point(5.0, 0.0, 0.0)), Some(point(6.0, 0.0, 0.0))]
        })
    );
    assert_eq!(app.document.objects().len(), 1);
    app.accept_drafting_point(point(6.0, 1.0, 0.0));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().nth(1).unwrap().geometry(),
        Geometry::Arc(_)
    ));
    assert_eq!(app.document.undo_label(), Some("Arc"));
}
