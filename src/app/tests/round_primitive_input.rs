use super::*;

#[test]
fn interactive_three_point_circle_retains_invalid_final_pick() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Circle _3Point"));
    assert!(app.accept_drafting_point(point(4.0, 0.0, 0.0)));
    assert!(app.accept_drafting_point(point(0.0, 4.0, 0.0)));
    assert!(!app.accept_drafting_point(point(-4.0, 8.0, 0.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleThreePoint {
            points: [Some(_), Some(_)]
        })
    ));
    assert!(app.accept_drafting_point(point(-4.0, 0.0, 0.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Circle(circle) if circle.center().is_near(point(0.0, 0.0, 0.0), Tolerance::DEFAULT)
            && (circle.radius() - 4.0).abs() < 1e-12
    ));
    assert_eq!(app.document.undo_label(), Some("Circle"));

    assert!(app.try_start_interactive_command("Circle"));
    app.command_input = "3Point".into();
    app.run_command();
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::CircleThreePoint { points: [None; 2] })
    );
    app.cancel_interactive_command(false);
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn interactive_two_point_circle_uses_diameter_and_retries_invalid_second_pick() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Circle 2Point"));
    assert!(app.accept_drafting_point(point(-4.0, 0.0, 0.0)));
    assert!(!app.accept_drafting_point(point(-4.0, 0.0, 0.0)));
    assert!(!app.accept_drafting_point(point(-4.0, 0.0, 4.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleTwoPoint { first: Some(_) })
    ));
    assert!(app.accept_drafting_point(point(4.0, 0.0, 0.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Circle(circle) if circle.center() == point(0.0, 0.0, 0.0)
            && circle.radius() == 4.0
            && circle.point_at_angle(0.0).unwrap() == point(4.0, 0.0, 0.0)
    ));

    assert!(app.try_start_interactive_command("Circle"));
    app.command_input = "2Point".into();
    app.run_command();
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::CircleTwoPoint { first: None })
    );
    app.cancel_interactive_command(false);
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn interactive_sphere_uses_center_and_radius_point_transactionally() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Sph"));
    let center = point(1.0, 2.0, 3.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(center);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Sphere {
            center: Some(center)
        })
    );
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.command_log.back().unwrap().contains("sphere radius"));

    app.accept_drafting_point(point(4.0, 2.0, 3.0));
    assert_eq!(app.active_command, None);
    let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created NURBS sphere")
    };
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 5);
    assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(4.0, 2.0, 3.0));
    assert_eq!(app.document.undo_label(), Some("Sphere"));
}

#[test]
fn interactive_ellipsoid_validates_each_axis_and_uses_command_history() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Ellipsoid"));
    let center = point(1.0, 2.0, 3.0);
    let first_axis = point(3.0, 2.0, 3.0);
    let second_axis = point(1.0, 5.0, 3.0);

    app.accept_drafting_point(center);
    app.accept_drafting_point(center);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Ellipsoid {
            points: [Some(center), None, None]
        })
    );
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(first_axis);
    let command = app.active_command.unwrap();
    assert_eq!(command.anchor(), Some(center));
    assert_eq!(command.reference(), Some(first_axis));
    app.accept_drafting_point(point(5.0, 2.0, 3.0));
    assert_eq!(app.active_command, Some(command));
    assert!(app.command_log.back().unwrap().contains("coordinate frame"));

    app.accept_drafting_point(second_axis);
    let command = app.active_command.unwrap();
    assert_eq!(command.anchor(), Some(center));
    assert_eq!(command.reference(), Some(second_axis));
    app.accept_drafting_point(center);
    assert_eq!(app.active_command, Some(command));
    assert_eq!(app.document.objects().len(), 0);
    app.accept_drafting_point(point(3.0, 4.0, 3.0));
    assert_eq!(app.active_command, Some(command));
    assert_eq!(app.document.objects().len(), 0);

    app.accept_drafting_point(point(1.0, 2.0, 7.0));
    assert_eq!(app.active_command, None);
    let Geometry::NurbsSurface(surface) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactively created NURBS ellipsoid")
    };
    assert_eq!(surface.control_point_count_u(), 9);
    assert_eq!(surface.control_point_count_v(), 5);
    assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), first_axis);
    assert_eq!(
        surface.evaluate(std::f64::consts::FRAC_PI_2, 0.0).unwrap(),
        second_axis
    );
    assert_eq!(
        surface.evaluate(0.0, std::f64::consts::FRAC_PI_2).unwrap(),
        point(1.0, 2.0, 7.0)
    );
    assert_eq!(app.document.undo_label(), Some("Ellipsoid"));
}
