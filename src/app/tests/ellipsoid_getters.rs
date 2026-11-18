use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use viboceros_geometry::NurbsSurface;

fn ellipsoid(app: &VibocerosApp) -> &NurbsSurface {
    app.document
        .objects()
        .find_map(|object| match object.geometry() {
            Geometry::NurbsSurface(surface) => Some(surface),
            _ => None,
        })
        .expect("expected a NURBS ellipsoid")
}

fn close(a: Point3, b: Point3) {
    assert!(a.distance_to(b).unwrap() < 1e-5, "{a:?} != {b:?}");
}

#[test]
fn solid_ellipsoid_axis_hover_matches_the_accepted_radius() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    enter(&mut app, "Ellipsoid");
    enter(&mut app, "0");
    enter(&mut app, "0.5cm");
    enter(&mut app, "w4,0,0");
    enter(&mut app, "3mm");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let second = screen_point(&app, point(0., 6., 0.));
    let wires = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(second)]);
    assert_eq!(wires.len(), 64);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, second);
    assert!(app.ellipsoid_session.is_some());

    app.viewports[0] = Viewport::new(ViewKind::Front);
    viewport_frame(&mut app, &context, vec![]);
    let first = screen_point(&app, point(0., 0., 2.));
    let initial = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(first)]);
    let third = screen_point(&app, point(8., 0., -4.));
    let preview = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(third)]);
    assert_eq!(preview.len(), 192);
    assert_ne!(preview, initial);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, third);
    let surface = ellipsoid(&app);
    close(
        surface.control_point(0, 0).unwrap().point(),
        point(-5., 0., 0.),
    );
    close(surface.evaluate(0., 0.).unwrap(), point(0., 3., 0.));
    close(
        surface.evaluate(std::f64::consts::FRAC_PI_2, 0.).unwrap(),
        point(0., 0., 4.),
    );
    assert_eq!(app.document.undo_label(), Some("Ellipsoid"));
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert!(app.document.objects().next().is_some());
}

#[test]
fn solid_ellipsoid_corner_diameter_and_vertical_stages_match_native_picks() {
    let cases: &[(&[&str], Point3, Point3)] = &[
        (
            &["Ellipsoid Corner", "w-5,-3,0", "w5,3,0", "2"],
            point(0., 3., 0.),
            point(0., 0., 2.),
        ),
        (
            &["Ellipsoid", "w0,0,0", "Corner", "w5,3,0", "-2"],
            point(0., 3., 0.),
            point(0., 0., 2.),
        ),
        (
            &["Ellipsoid Diameter", "w-5,0,0", "w5,0,0", "w0,3,0", "4"],
            point(0., 3., 0.),
            point(0., 0., 2.),
        ),
        (
            &[
                "Ellipsoid",
                "Diameter",
                "Vertical",
                "w-5,0,0",
                "w5,0,0",
                "w0,0,3",
                "4",
            ],
            point(0., 0., 3.),
            point(0., -2., 0.),
        ),
    ];
    for &(steps, second, third) in cases {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        for step in steps {
            enter(&mut app, step);
        }
        assert!(
            app.active_command.is_none(),
            "{steps:?}: {:?}",
            app.command_log
        );
        let surface = ellipsoid(&app);
        close(
            surface.control_point(0, 0).unwrap().point(),
            point(-5., 0., 0.),
        );
        close(surface.evaluate(0., 0.).unwrap(), second);
        close(
            surface.evaluate(std::f64::consts::FRAC_PI_2, 0.).unwrap(),
            third,
        );
    }
}

#[test]
fn solid_ellipsoid_focal_preview_marks_authored_points_in_one_edit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    for step in ["Ellipsoid FromFoci", "w4,0,0", "w-4,0,0", "MarkFoci=Yes"] {
        enter(&mut app, step);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let cursor = screen_point(&app, point(0., 3., 0.));
    let wires = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
    assert_eq!(wires.len(), 192);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, cursor);
    assert!(app.active_command.is_none());
    let markers = app
        .document
        .objects()
        .skip(1)
        .map(|object| match object.geometry() {
            Geometry::Point(point) => *point,
            _ => panic!("expected focus point"),
        })
        .collect::<Vec<_>>();
    assert_eq!(markers, [point(4., 0., 0.), point(-4., 0., 0.)]);
    assert!(app.commands.ellipsoid_mark_foci());
    assert!(!app.commands.ellipse_options().mark_foci);
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn solid_ellipsoid_around_curve_keeps_the_guide_through_history() {
    for preselected in [false, true] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Front);
        app.maximized_viewport = Some(0);
        app.osnap = false;
        enter(&mut app, "Line 0,0,0 0,10,10");
        let guide = app.document.objects().next().unwrap().clone();
        let id = guide.id();
        if preselected {
            app.document
                .select_object(id, SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "Ellipsoid AroundCurve");
        if !preselected {
            enter(&mut app, &format!("Curve={id}"));
        }
        for step in ["w0,3,3", "w5,3,3"] {
            enter(&mut app, step);
        }
        let context = egui::Context::default();
        viewport_frame(&mut app, &context, vec![]);
        let minor = screen_point(&app, point(0., 5.121320343559643, 0.8786796564403576));
        let base = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(minor)]);
        assert_eq!(base.len(), 64);
        viewport_click(&mut app, &context, minor);
        assert!(app.ellipsoid_session.is_some());
        assert!(app.document.is_selected(id));
        enter(&mut app, "2mm");
        let surface = ellipsoid(&app).clone();
        close(
            surface.control_point(0, 0).unwrap().point(),
            point(-5., 3., 3.),
        );
        close(
            surface.evaluate(std::f64::consts::FRAC_PI_2, 0.).unwrap(),
            point(0., 4.414213562373095, 4.414213562373095),
        );
        assert_eq!(app.document.object(id), Some(&guide));
        assert_eq!(app.document.is_selected(id), preselected);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().count(), 1);
        assert_eq!(app.document.is_selected(id), preselected);
        enter(&mut app, "Redo");
        assert_eq!(ellipsoid(&app), &surface);
        assert_eq!(app.document.is_selected(id), preselected);
    }
}

#[test]
fn solid_ellipsoid_cancels_if_the_guide_is_deleted_during_the_third_axis() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 0,0,10");
    let guide = app.document.objects().next().unwrap().id();
    enter(&mut app, "Ellipsoid AroundCurve");
    enter(&mut app, &format!("Curve={guide}"));
    for step in ["w0,0,4", "w5,0,4", "w0,3,4"] {
        enter(&mut app, step);
    }
    assert!(app.ellipsoid_session.is_some());
    app.document.delete_object(guide).unwrap();
    assert!(app.ellipsoid_preview().is_none());
    enter(&mut app, "2");
    assert!(app.active_command.is_none());
    assert!(app.document.objects().next().is_none());
    assert!(app.command_log.back().unwrap().contains("guide changed"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), 1);
}
