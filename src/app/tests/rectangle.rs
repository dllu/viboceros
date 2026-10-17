use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn rectangle_numeric_width_waits_for_orientation_and_commits_one_edit() {
    let mut app = test_app();
    enter(&mut app, "Rectangle 3Point");
    let first = point(1., 2., 3.);
    assert!(app.accept_drafting_point(first));
    enter(&mut app, "10mm");
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.accept_drafting_point(point(4., 6., 8.)));
    enter(&mut app, "5mm");
    let getter = app.rectangle_getter().unwrap().input;
    let before = format!("{:?}", app.document);
    let choice = point(7., -1., 3.);
    let expected = getter
        .preview(choice)
        .unwrap()
        .polyline(app.document.tolerance())
        .unwrap();
    assert!(
        (expected.vertices()[0]
            .distance_to(expected.vertices()[1])
            .unwrap()
            - 10.)
            .abs()
            < 1e-10
    );
    assert!(
        (expected.vertices()[1]
            .distance_to(expected.vertices()[2])
            .unwrap()
            - 5.)
            .abs()
            < 1e-10
    );
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.accept_drafting_point(choice));
    assert!(app.rectangle_session.is_none());
    let Geometry::Polyline(actual) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    for (a, b) in actual.vertices().iter().zip(expected.vertices()) {
        assert!(a.distance_to(*b).unwrap() < 1e-10);
    }
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Rectangle"));
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    let Geometry::Polyline(actual) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    for (a, b) in actual.vertices().iter().zip(expected.vertices()) {
        assert!(a.distance_to(*b).unwrap() < 1e-10);
    }
}
#[test]
fn rectangle_center_vertical_and_enter_defaults_use_shared_getters() {
    let mut app = test_app();
    for (start, inputs, last) in [
        ("Rect Center", vec!["1,2,3", "10", ""], None),
        (
            "Rectangle Vertical",
            vec!["1,2,3", "5,8,9", ""],
            Some(point(1., 2., 10.)),
        ),
        (
            "Rectangle Center",
            vec!["1,2,3", "3Point", "4,6,8"],
            Some(point(7., -1., 3.)),
        ),
    ] {
        enter(&mut app, start);
        for text in inputs {
            enter(&mut app, text);
        }
        if let Some(last) = last {
            let expected = app.rectangle_getter().unwrap().input.preview(last).unwrap();
            assert!(app.accept_drafting_point(last));
            let Geometry::Polyline(actual) = app.document.objects().last().unwrap().geometry()
            else {
                panic!()
            };
            for (a, b) in actual.vertices().iter().zip(expected.corners) {
                assert!(a.distance_to(b).unwrap() < 1e-10);
            }
        }
        assert!(app.rectangle_session.is_none());
    }
    assert_eq!(app.document.objects().count(), 3);
    enter(&mut app, "Rectangle Center");
    enter(&mut app, "1,2,3");
    enter(&mut app, "10");
    let before = format!("{:?}", app.document);
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
}
#[test]
fn rectangle_around_curve_and_edges_capture_without_model_edits_and_reject_stale_references() {
    let mut app = test_app();
    let guide = app
        .document
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(
                point(0., 0., 0.),
                point(0., 0., 10.),
                app.document.tolerance(),
            )
            .unwrap(),
        ))
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Rectangle AroundCurve");
    assert!(app.picking_rectangle_curve());
    assert_eq!(
        app.viewport_object_filter(),
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    );
    app.pick_rectangle_curve(Some(guide), None);
    assert!(!app.picking_rectangle_curve());
    app.document.set_objects_visibility([guide], false).unwrap();
    app.validate_rectangle_curve();
    assert!(app.picking_rectangle_curve());
    assert_eq!(app.accept_rectangle_point(point(0., 0., 5.)), Some(false));
    app.document.set_objects_visibility([guide], true).unwrap();
    app.pick_rectangle_curve(Some(guide), None);
    assert!(app.accept_rectangle_parameter(5.));
    enter(&mut app, "8");
    enter(&mut app, "6");
    let Geometry::Polyline(actual) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(actual.vertices().iter().all(|p| (p.z() - 5.).abs() < 1e-9));
    assert_eq!(actual.vertices()[0], point(-4., -3., 5.));
    assert_eq!(app.document.objects().count(), 2);
    // Capture the boundary of a real surface through the component route.
    let surface = app
        .document
        .add_geometry(Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                point(0., 0., 0.),
                point(10., 0., 0.),
                point(10., 10., 0.),
                point(0., 10., 0.),
            ])
            .unwrap(),
        ))
        .unwrap();
    enter(&mut app, "Rectangle AroundCurve");
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: surface,
            index: 0,
            kind: viboceros_command::ComponentSelectionKind::BrepEdge,
        }],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    assert!(!app.picking_rectangle_curve());
    assert!(app.rectangle_getter().unwrap().cursor_curve().is_some());
    let before = format!("{:?}", app.document);
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
}

#[test]
fn rectangle_latches_the_first_pick_viewport_and_releases_the_plane_on_completion() {
    let mut app = test_app();
    enter(&mut app, "Rectangle");
    app.active_viewport = 2;
    enter(&mut app, "0");
    app.active_viewport = 0;
    enter(&mut app, "w4,8,5");
    assert!(app.rectangle_session.is_none());
    assert!(app.drafting_plane.is_none());
    let Geometry::Polyline(line) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(
        line.vertices(),
        &[
            point(0., 0., 0.),
            point(4., 0., 0.),
            point(4., 0., 5.),
            point(0., 0., 5.),
            point(0., 0., 0.)
        ]
    );
}

#[test]
fn rounded_rectangle_previews_then_creates_one_history_edit_with_correct_defaults() {
    let mut app = test_app();
    enter(&mut app, "Rectangle Rounded");
    enter(&mut app, "0");
    let before = format!("{:?}", app.document);
    let preview = app
        .rectangle_getter()
        .unwrap()
        .input
        .preview(point(10., 6., 0.))
        .unwrap();
    assert!(preview.rounding.is_some());
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "10,6,0");
    assert!(app.rectangle_getter().unwrap().input.rounding_pending());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "-2mm");
    assert_eq!(app.commands.rectangle_options().radius, 2.);
    assert_eq!(app.document.objects().count(), 1);
    assert!(app.rectangle_session.is_none());
    let original = app.document.objects().next().unwrap().geometry().clone();
    assert!(matches!(original, Geometry::PolyCurve(_)));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Rectangle"));
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().next().unwrap().geometry(), &original);
    enter(&mut app, "Rectangle Rounded");
    enter(&mut app, "0");
    enter(&mut app, "10,6,0");
    enter(&mut app, "");
    assert_eq!(app.document.objects().last().unwrap().geometry(), &original);
    enter(&mut app, "Rectangle");
    enter(&mut app, "0");
    enter(&mut app, "10,6,0");
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Polyline(_)
    ));
}

#[test]
fn conic_rectangle_dimensionless_input_retry_point_preview_and_cancellation_are_atomic() {
    let mut app = test_app();
    enter(&mut app, "Rectangle Rounded Center");
    enter(&mut app, "0");
    enter(&mut app, "10");
    enter(&mut app, "6");
    enter(&mut app, "Corner=Conic");
    let before = format!("{:?}", app.document);
    for input in ["0", "1", "0.5mm"] {
        enter(&mut app, input);
        assert_eq!(format!("{:?}", app.document), before);
        assert!(app.rectangle_getter().unwrap().input.rho_pending());
    }
    enter(&mut app, "1/2");
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.commands.rectangle_options().rho, 0.5);
    enter(&mut app, "Rectangle Rounded");
    enter(&mut app, "0");
    enter(&mut app, "10,6,0");
    assert!(app.rectangle_getter().unwrap().input.rho_pending());
    let getter = app.rectangle_getter().unwrap().input;
    let p = point(1., 1., 3.);
    let preview = getter
        .preview(p)
        .unwrap()
        .geometry(app.document.tolerance())
        .unwrap();
    let before = format!("{:?}", app.document);
    assert_eq!(format!("{:?}", app.document), before);
    app.accept_drafting_point(p);
    assert_eq!(app.document.objects().last().unwrap().geometry(), &preview);
    enter(&mut app, "Rectangle Rounded");
    enter(&mut app, "0");
    enter(&mut app, "10,6,0");
    enter(&mut app, "Corner");
    assert!(!app.rectangle_getter().unwrap().input.rho_pending());
    let before = format!("{:?}", app.document);
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
    assert_eq!(
        app.commands.rectangle_options().kind,
        viboceros_command::rectangle_rounding::Kind::Arc
    );
}
