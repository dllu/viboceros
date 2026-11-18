use super::*;

#[test]
fn circle_size_option_works_through_the_command_line() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "0,0,0");
    enter(&mut app, "Diameter");
    enter(&mut app, "6");
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 3.0).abs() < 1e-12
    ));
}

#[test]
fn vertical_circle_uses_a_direction_pick_after_numeric_radius() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "Vertical");
    enter(&mut app, "1,2,3");
    enter(&mut app, "4");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleVertical {
            center: Some(_),
            radius: Some(4.0),
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(9.0, 2.0, 3.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
            && circle.point_at_angle(0.0).unwrap() == point(5.0, 2.0, 3.0)
    ));

    enter(&mut app, "Circle Vertical");
    assert!(app.accept_drafting_point(point(1.0, 2.0, 3.0)));
    assert!(!app.accept_drafting_point(point(1.0, 2.0, 7.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleVertical {
            center: Some(_),
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(5.0, 2.0, 5.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 20.0_f64.sqrt()).abs() < 1e-12
    ));

    enter(&mut app, "Circle Vertical");
    assert!(app.accept_drafting_point(point(1.0, 2.0, 3.0)));
    enter(&mut app, "Diameter 8");
    assert!(app.accept_drafting_point(point(9.0, 2.0, 3.0)));
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
    ));
}

#[test]
fn oriented_circle_accepts_normal_then_numeric_or_projected_radius() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "1,2,3");
    enter(&mut app, "Orientation");
    assert!(!app.accept_drafting_point(point(1.0, 2.0, 3.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleOrientation {
            normal_point: None,
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(1.0, 3.0, 3.0)));
    enter(&mut app, "4");
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
            && circle.point_at_angle(0.0).unwrap() == point(1.0, 2.0, 7.0)
    ));

    enter(&mut app, "Circle");
    enter(&mut app, "1,2,3");
    enter(&mut app, "Orientation");
    assert!(app.accept_drafting_point(point(1.0, 3.0, 3.0)));
    assert!(app.accept_drafting_point(point(5.0, 4.0, 3.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
            && circle.point_at_angle(0.0).unwrap() == point(5.0, 2.0, 3.0)
    ));

    enter(&mut app, "Circle");
    enter(&mut app, "1,2,3");
    enter(&mut app, "Orientation");
    assert!(app.accept_drafting_point(point(1.0, 3.0, 3.0)));
    enter(&mut app, "Diameter");
    assert!(app.accept_drafting_point(point(5.0, 4.0, 3.0)));
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
            && circle.point_at_angle(0.0).unwrap() == point(5.0, 2.0, 3.0)
    ));

    for (mode, expected_radius) in [
        ("Circumference", 20.0_f64.sqrt() / std::f64::consts::TAU),
        ("Area", (20.0_f64.sqrt() / std::f64::consts::PI).sqrt()),
    ] {
        enter(&mut app, "Circle");
        enter(&mut app, "1,2,3");
        enter(&mut app, "Orientation");
        assert!(app.accept_drafting_point(point(1.0, 3.0, 3.0)));
        enter(&mut app, mode);
        assert!(app.accept_drafting_point(point(5.0, 4.0, 3.0)));
        assert_eq!(app.active_command, None);
        assert!(matches!(
            app.document.objects().last().unwrap().geometry(),
            Geometry::Circle(circle) if (circle.radius() - expected_radius).abs() < 1e-12
                && (circle.point_at_angle(0.0).unwrap().z() - (3.0 + expected_radius)).abs() < 1e-12
        ));
    }
}

#[test]
fn three_point_circle_radius_prompt_accepts_number_or_location() {
    let mut app = test_app();
    enter(&mut app, "Circle 3Point");
    assert!(app.accept_drafting_point(point(4.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(0.0, 4.0, 0.0)));
    enter(&mut app, "Radius");
    enter(&mut app, "5");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleThreePointRadius {
            radius: Some(5.0),
            ..
        })
    ));
    assert!(!app.accept_drafting_point(point(2.0, 2.0, 0.0)));
    assert!(app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 5.0).abs() < 1e-12
            && circle.point_at_angle(0.0).unwrap().is_near(point(0.0, 4.0, 0.0), Tolerance::DEFAULT)
    ));

    enter(&mut app, "Circle 3Point");
    assert!(app.accept_drafting_point(point(4.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(0.0, 4.0, 0.0)));
    enter(&mut app, "Radius");
    assert!(app.accept_drafting_point(point(5.0, 4.0, 0.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 17.0_f64.sqrt()).abs() < 1e-12
            && circle.point_at_angle(0.0).unwrap().is_near(point(0.0, 4.0, 0.0), Tolerance::DEFAULT)
    ));

    enter(&mut app, "Circle 3Point");
    assert!(app.accept_drafting_point(point(4.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(0.0, 4.0, 0.0)));
    enter(&mut app, "Radius=5");
    assert!(app.accept_drafting_point(point(4.0, 4.0, 0.0)));
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 5.0).abs() < 1e-12
    ));
}

#[test]
fn pipe_picks_rail_and_radius_or_uses_preselected_rail() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let source = app.document.objects().next().unwrap().id();
    enter(&mut app, "Pipe Cap=None");
    assert!(app.picking_pipe_rail());
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    assert!(!app.picking_pipe_rail());
    assert!(app.accept_drafting_point(point(0., 1., 0.)));
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if !brep.is_closed() && brep.faces().len()==1
    ));

    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pipe 0.5 Cap=Flat");
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed()
    ));

    enter(&mut app, "Pipe Cap=Flat Thick=Yes WallThickness=0.2");
    let session = app.pipe_session.as_ref().unwrap();
    assert_eq!(session.rails[0].references().next(), Some((source, None)));
    assert!(app.accept_drafting_point(point(0., 1., 0.)));
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed() && brep.faces().len() == 4
    ));

    enter(&mut app, "Pipe Thick=Yes");
    assert!(app.accept_drafting_point(point(0., 1., 0.)));
    assert!(app.accept_drafting_point(point(0., 1.2, 0.)));
    enter(&mut app, "");
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed() && brep.faces().len() == 4
    ));

    enter(&mut app, "Pipe Cap=Round Thick=No");
    assert!(app.accept_drafting_point(point(0., 1., 0.)));
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed() && brep.is_solid() && brep.faces().len() == 3
    ));
}

#[test]
fn pipe_accepts_options_while_picking_without_losing_radius_state() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let source = app.document.objects().next().unwrap().id();
    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pipe");
    enter(&mut app, "Thick=Yes");
    assert!(app.accept_drafting_point(point(0., 1., 0.)));
    enter(&mut app, "Cap=Round");
    enter(&mut app, "ShapeBlending=Global");
    enter(&mut app, "FitRail=Yes");
    enter(&mut app, "Cap=Invalid");
    let (input, _) = app.pipe_session.as_ref().unwrap().preview().unwrap();
    let prompt = input.prompt();
    assert!(prompt.starts_with("Second start radius"));
    assert!(prompt.contains("Cap=Round"));
    assert!(prompt.contains("ShapeBlending=Global"));
    assert!(prompt.contains("FitRail=Yes"));
    assert!(app.accept_drafting_point(point(0., 1.2, 0.)));
    enter(&mut app, "");
    enter(&mut app, "");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed() && brep.is_solid() && brep.faces().len() == 4
    ));
}
