use super::*;

#[test]
fn interactive_ellipse_and_polygon_validate_each_pick() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Ell"));
    let center = point(0.0, 0.0, 2.0);
    let first_axis = point(4.0, 0.0, 2.0);
    app.accept_drafting_point(center);
    app.accept_drafting_point(center);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Ellipse {
            center: Some(center),
            first_axis: None,
        })
    );
    app.accept_drafting_point(first_axis);
    app.accept_drafting_point(point(2.0, 0.0, 2.0));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Ellipse {
            center: Some(center),
            first_axis: Some(first_axis),
        })
    );
    app.accept_drafting_point(point(0.0, 3.0, 2.0));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Ellipse(ellipse)
            if ellipse.radius_x() == 4.0 && ellipse.radius_y() == 3.0
    ));
    assert_eq!(app.document.undo_label(), Some("Ellipse"));

    assert!(app.try_start_interactive_command("Polygon 6"));
    let polygon_center = point(10.0, 10.0, 5.0);
    app.accept_drafting_point(polygon_center);
    app.accept_drafting_point(polygon_center);
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Polygon {
            side_count: 6,
            center: Some(polygon_center),
        })
    );
    app.accept_drafting_point(point(12.0, 10.0, 5.0));
    assert_eq!(app.active_command, None);
    let Geometry::Polyline(polygon) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!("expected an interactive polygon")
    };
    assert!(polygon.is_closed());
    assert_eq!(polygon.segment_count(), 6);
    assert_eq!(app.document.undo_label(), Some("Polygon"));

    assert!(!app.try_start_interactive_command("Polygon 2"));
    assert!(app.try_start_interactive_command("Polygon"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Polygon {
            side_count: 4,
            center: None,
        })
    );
}

#[test]
fn interactive_polyline_collects_until_enter_and_cancel_discards_vertices() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("PLine"));
    app.run_command();
    assert_eq!(app.active_command, Some(InteractiveCommand::Polyline));
    assert_eq!(app.document.objects().len(), 0);

    let first = point(0.0, 0.0, 1.0);
    let second = point(3.0, 0.0, 1.0);
    let third = point(3.0, 2.0, 1.0);
    app.accept_drafting_point(first);
    app.accept_drafting_point(first);
    assert_eq!(app.curve_points, vec![first]);
    app.accept_drafting_point(second);
    app.accept_drafting_point(third);
    app.accept_drafting_point(first);
    assert_eq!(app.curve_points.last(), Some(&first));

    app.run_command();
    assert_eq!(app.active_command, None);
    assert!(app.curve_points.is_empty());
    let Geometry::Polyline(polyline) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactive polyline")
    };
    assert!(polyline.is_closed());
    assert_eq!(polyline.segment_count(), 3);
    assert_eq!(app.document.undo_label(), Some("Polyline"));

    assert!(app.try_start_interactive_command("Polyline"));
    app.accept_drafting_point(point(5.0, 5.0, 0.0));
    app.cancel_interactive_command(false);
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn interactive_curve_collects_control_points_and_preserves_degree_option() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("Curve Degree=5"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Curve {
            degree: 5,
            closure: ControlPointCurveClosure::Open,
        })
    );

    let controls = [
        point(0.0, 0.0, 0.0),
        point(2.0, 3.0, 0.0),
        point(10.0, 0.0, 0.0),
    ];
    app.accept_drafting_point(controls[0]);
    app.accept_drafting_point(controls[0]);
    assert_eq!(app.curve_points, vec![controls[0]]);
    for control in &controls[1..] {
        app.accept_drafting_point(*control);
    }
    app.run_command();

    assert_eq!(app.active_command, None);
    assert!(app.curve_points.is_empty());
    let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactive control-point curve")
    };
    assert_eq!(curve.degree(), 2);
    assert_eq!(
        curve
            .control_points()
            .iter()
            .map(|control| control.point())
            .collect::<Vec<_>>(),
        controls
    );
    assert_eq!(app.document.undo_label(), Some("Curve"));

    assert!(app.try_start_interactive_command("Curve Close=Smooth Degree=3"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Curve {
            degree: 3,
            closure: ControlPointCurveClosure::Smooth,
        })
    );
    let periodic_controls = [
        point(20.0, 0.0, 0.0),
        point(22.0, 3.0, 0.0),
        point(25.0, 0.0, 0.0),
        point(22.0, -2.0, 0.0),
    ];
    for control in &periodic_controls[..2] {
        app.accept_drafting_point(*control);
    }
    app.run_command();
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Curve {
            closure: ControlPointCurveClosure::Smooth,
            ..
        })
    ));
    assert_eq!(app.document.objects().len(), 1);
    for control in &periodic_controls[2..] {
        app.accept_drafting_point(*control);
    }
    app.run_command();
    let Geometry::NurbsCurve(periodic) = app.document.objects().nth(1).unwrap().geometry() else {
        panic!("expected an interactive periodic control-point curve")
    };
    assert!(periodic.is_periodic());
    assert!(periodic.is_closed().unwrap());

    assert!(app.try_start_interactive_command("Curve Degree=15"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::Curve {
            degree: 11,
            closure: ControlPointCurveClosure::Open,
        })
    );
    app.accept_drafting_point(point(20.0, 0.0, 0.0));
    app.cancel_interactive_command(false);
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn interactive_interp_crv_collects_points_until_enter() {
    let mut app = test_app();
    assert!(app.try_start_interactive_command("InterpCurve"));
    assert_eq!(
        app.active_command,
        Some(InteractiveCommand::InterpCrv {
            options: Default::default()
        })
    );

    let points = [
        point(0.0, 0.0, 0.0),
        point(1.0, 2.0, 0.0),
        point(4.0, -1.0, 0.0),
        point(6.0, 0.0, 0.0),
    ];
    app.accept_drafting_point(points[0]);
    app.accept_drafting_point(points[0]);
    assert_eq!(app.curve_points, vec![points[0]]);
    for point in &points[1..] {
        app.accept_drafting_point(*point);
    }
    app.run_command();

    assert_eq!(app.active_command, None);
    assert!(app.curve_points.is_empty());
    let Geometry::NurbsCurve(curve) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected an interactive interpolated curve")
    };
    let mut parameter = 0.0;
    for (index, expected) in points.into_iter().enumerate() {
        assert!(
            curve
                .evaluate(parameter)
                .unwrap()
                .is_near(expected, Tolerance::DEFAULT)
        );
        if let Some(next) = points.get(index + 1) {
            parameter += expected.distance_to(*next).unwrap();
        }
    }
    assert_eq!(app.document.undo_label(), Some("InterpCrv"));

    assert!(app.try_start_interactive_command("InterpCrv"));
    app.accept_drafting_point(point(10.0, 10.0, 0.0));
    app.cancel_interactive_command(false);
    assert!(app.curve_points.is_empty());
    assert_eq!(app.document.objects().len(), 1);
}
