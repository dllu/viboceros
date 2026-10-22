use super::*;

#[test]
fn interactive_curve_extrusion_uses_two_direction_points_and_options() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ExtrudeCrv BothSides=Yes DeleteInput=No"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeCurve {
            base: None,
            both_sides: true,
            delete_input: false,
        })
    );
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeCurve {
            base: Some(point(0.0, 0.0, 0.0)),
            both_sides: true,
            delete_input: false,
        })
    );
    app.accept_drafting_point(point(0.0, 2.0, 0.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.is_selected(source));
    let output = app.document.objects().nth(1).unwrap();
    assert!(!app.document.is_selected(output.id()));
    let surface = output
        .geometry()
        .nurbs_surface_representation()
        .unwrap()
        .unwrap();
    assert_eq!(surface.domain_v(), 0.0..=4.0);
    assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(0.0, -2.0, 0.0));
    assert_eq!(surface.evaluate(0.0, 4.0).unwrap(), point(0.0, 2.0, 0.0));
    assert_eq!(app.document.undo_label(), Some("ExtrudeCrv"));
    assert!(!app.try_start_interactive_command("ExtrudeCrv BothSides=Maybe"));
}

#[test]
fn interactive_curve_to_point_extrusion_uses_one_apex_pick() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(0.0, 0.0, 0.0),
                point(4.0, 0.0, 0.0),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("ExtrudeCrvToPoint DeleteInput=No"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeCurveToPoint {
            delete_input: false,
            solid: false,
        })
    );
    app.accept_drafting_point(point(1.0, 2.0, 5.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.is_selected(source));
    let output = app.document.objects().nth(1).unwrap();
    assert!(!app.document.is_selected(output.id()));
    let Geometry::NurbsSurface(surface) = output.geometry() else {
        panic!("expected a curve-to-point NURBS surface")
    };
    assert_eq!(surface.degree_u(), 1);
    assert_eq!(surface.degree_v(), 1);
    assert_eq!(surface.domain_v(), 0.0..=4.0);
    let apex = point(1.0, 2.0, 5.0);
    assert_eq!(
        surface.evaluate(*surface.domain_u().end(), 2.0).unwrap(),
        apex
    );
    assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
    assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint DeleteInput=Maybe"));
}

#[test]
fn interactive_curve_to_point_preserves_solid_option_through_apex_pick() {
    let mut app = test_app();
    let rectangle = viboceros_geometry::Polyline3::try_new(
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 3.0, 0.0),
            point(0.0, 3.0, 0.0),
            point(0.0, 0.0, 0.0),
        ],
        app.document.tolerance(),
    )
    .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::Polyline(rectangle))
        .unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command(
        "ExtrudeCrvToPoint Solid=Yes Output=Surface DeleteInput=No"
    ));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::ExtrudeCurveToPoint {
            delete_input: false,
            solid: true,
        })
    );
    app.accept_drafting_point(point(1.0, 2.0, 6.0));
    assert_eq!(app.active_command, None);
    let output = app.document.objects().nth(1).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("expected an interactive capped apex B-rep")
    };
    assert!(brep.is_solid());
    assert!((brep.signed_volume(app.document.tolerance()).unwrap() - 12.0).abs() < 1.0e-11);
    assert_eq!(app.document.undo_label(), Some("ExtrudeCrvToPoint"));
    assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint Solid=Yes Solid=No"));
    assert!(!app.try_start_interactive_command("ExtrudeCrvToPoint Output=SubD"));
}

#[test]
fn interactive_revolve_uses_two_axis_picks_and_angle_options() {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(2.0, 0.0, 0.0),
                point(2.0, 0.0, 3.0),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document
        .select_object(source, viboceros_document::SelectionMode::Replace)
        .unwrap();

    assert!(app.try_start_interactive_command("Revolve Angle=120 StartAngle=30 DeleteInput=No"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Revolve {
            axis_start: None,
            start_angle_degrees: 30.0,
            sweep_degrees: 120.0,
            delete_input: false,
        })
    );
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Revolve {
            axis_start: Some(point(0.0, 0.0, 0.0)),
            start_angle_degrees: 30.0,
            sweep_degrees: 120.0,
            delete_input: false,
        })
    );
    app.accept_drafting_point(point(0.0, 0.0, 1.0));
    assert_eq!(app.active_command, None);
    assert_eq!(app.document.objects().len(), 2);
    assert!(app.document.is_selected(source));
    let output = app.document.objects().nth(1).unwrap();
    assert!(!app.document.is_selected(output.id()));
    let Geometry::NurbsSurface(surface) = output.geometry() else {
        panic!("expected a revolved NURBS surface")
    };
    assert_eq!(surface.control_point_count_u(), 5);
    assert_eq!(surface.domain_u(), 0.0..=2.0 * 120.0_f64.to_radians());
    assert!(
        surface
            .evaluate(0.0, 0.0)
            .unwrap()
            .is_near(point(3.0_f64.sqrt(), 1.0, 0.0), app.document.tolerance())
    );
    assert_eq!(app.document.undo_label(), Some("Revolve"));
    assert!(!app.try_start_interactive_command("Revolve Angle=0"));
    assert!(!app.try_start_interactive_command("Revolve Angle=361"));
    assert!(!app.try_start_interactive_command("Revolve DeleteInput=Maybe"));
}

#[test]
fn interactive_scale_rotate_and_mirror_use_reference_points() {
    let mut app = test_app();
    let object = app
        .document
        .add_geometry(Geometry::Point(point(2.0, 1.0, 0.0)))
        .unwrap();
    app.document
        .select_object(object, viboceros_document::SelectionMode::Replace)
        .unwrap();
    let position = |app: &VibocerosApp| match app.document.object(object).unwrap().geometry() {
        Geometry::Point(point) => *point,
        _ => panic!("expected a point"),
    };

    assert!(app.try_start_interactive_command("Scale"));
    app.accept_drafting_point(point(1.0, 1.0, 0.0));
    app.accept_drafting_point(point(1.0, 1.0, 0.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Scale {
            kind: InteractiveScaleKind::Uniform,
            center: Some(point(1.0, 1.0, 0.0)),
            reference: None,
        })
    );
    app.accept_drafting_point(point(2.0, 1.0, 0.0));
    app.accept_drafting_point(point(3.0, 1.0, 0.0));
    assert_eq!(position(&app), point(3.0, 1.0, 0.0));
    assert_eq!(app.document.undo_label(), Some("Scale"));
    app.document.undo().unwrap();

    let scale1d_history_before = app.document.undo_label().map(str::to_owned);
    assert!(app.try_start_interactive_command("Scale1D"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(1.0, 0.0, 0.0));
    // Native picked zero targets are rejected; numeric zero factors remain supported.
    assert!(!app.accept_drafting_point(point(0.0, 0.0, 0.0)));
    assert_eq!(position(&app), point(2.0, 1.0, 0.0));
    assert_eq!(app.document.undo_label(), scale1d_history_before.as_deref());
    assert!(app.accept_drafting_point(point(3.0, 5.0, 0.0)));
    assert_eq!(position(&app), point(6.0, 1.0, 0.0));
    assert_eq!(app.document.undo_label(), Some("Scale1D"));
    app.document.undo().unwrap();

    assert!(app.try_start_interactive_command("Scale2D"));
    app.accept_drafting_point(point(1.0, 1.0, 0.0));
    app.accept_drafting_point(point(2.0, 1.0, 0.0));
    app.accept_drafting_point(point(3.0, 1.0, 0.0));
    assert_eq!(position(&app), point(3.0, 1.0, 0.0));
    assert_eq!(app.document.undo_label(), Some("Scale2D"));
    app.document.undo().unwrap();

    assert!(app.try_start_interactive_command("Rotate"));
    app.accept_drafting_point(point(1.0, 1.0, 0.0));
    app.accept_drafting_point(point(2.0, 1.0, 0.0));
    app.accept_drafting_point(point(1.0, 2.0, 0.0));
    assert!(position(&app).is_near(point(1.0, 2.0, 0.0), app.document.tolerance()));
    assert_eq!(app.document.undo_label(), Some("Rotate"));
    app.document.undo().unwrap();

    assert!(app.try_start_interactive_command("Rotate3D"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Rotate3D {
            points: [Some(_), None, None]
        })
    ));
    app.accept_drafting_point(point(0.0, 0.0, 1.0));
    app.accept_drafting_point(point(0.0, 0.0, 2.0));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Rotate3D {
            points: [Some(_), Some(_), None]
        })
    ));
    app.accept_drafting_point(point(1.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 1.0, 0.0));
    assert!(position(&app).is_near(point(-1.0, 2.0, 0.0), app.document.tolerance()));
    assert_eq!(app.document.undo_label(), Some("Rotate3D"));
    app.document.undo().unwrap();

    assert!(app.try_start_interactive_command("Shear"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 0.0, 1.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Shear {
            origin: Some(point(0.0, 0.0, 0.0)),
            reference: None,
        })
    );
    app.accept_drafting_point(point(1.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Shear {
            origin: Some(_),
            reference: Some(_),
        })
    ));
    app.accept_drafting_point(point(1.0, 1.0, 0.0));
    assert!(position(&app).is_near(point(2.0, 3.0, 0.0), app.document.tolerance()));
    assert_eq!(app.document.undo_label(), Some("Shear"));
    app.document.undo().unwrap();

    assert!(app.try_start_interactive_command("Mirror"));
    app.accept_drafting_point(point(0.0, 0.0, 0.0));
    app.accept_drafting_point(point(0.0, 1.0, 0.0));
    assert_eq!(position(&app), point(2.0, 1.0, 0.0));
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.objects().nth(1).unwrap().geometry(),
        &Geometry::Point(point(-2.0, 1.0, 0.0))
    );
    assert_eq!(app.document.undo_label(), Some("Mirror"));
}
