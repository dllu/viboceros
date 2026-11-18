use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use viboceros_geometry::NurbsSurface;

fn surface(app: &VibocerosApp) -> &NurbsSurface {
    let Geometry::NurbsSurface(surface) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a NURBS sphere");
    };
    surface
}

#[test]
fn sphere_point_getters_preview_the_same_surface_they_commit() {
    // Centers and first equator points measured from the native 9-by-5 net.
    let cases: &[(&[&str], [f64; 3], [f64; 3], [f64; 3])] = &[
        (&["Sphere", "w0,0,0"], [0., 0., 2.], [0.; 3], [0., 0., 2.]),
        (
            &["Sphere", "2Point", "w0,0,0"],
            [1., 2., 3.],
            [0.5, 1., 1.5],
            [0.5, -0.1976753439510207, 0.06278958725877529],
        ),
        (
            &["Sph 2Point", "w0,0,-3"],
            [0., 0., 3.],
            [0.; 3],
            [0., 3., 0.],
        ),
        (
            &["Sphere 3Point", "w3,0,0", "w0,3,0"],
            [-3., 0., 0.],
            [0.; 3],
            [3., 0., 0.],
        ),
        (
            &["Sphere 3Point", "w-2,0,0", "w2,0,0", "Radius=3"],
            [0., 5., 0.],
            [0., 5.0_f64.sqrt(), 0.],
            [2., 0., 0.],
        ),
        (
            &["Sphere", "4Point", "w3,0,0", "w0,3,0", "w-3,0,0"],
            [0., 0., 6.],
            [0., 0., 2.25],
            [3.75, 0., 2.25],
        ),
        (
            &["Sphere 4Point", "w3,0,0", "w0,3,0", "w-3,0,0", "Radius 4"],
            [0., 0., 10.],
            [0., 0., 7.0_f64.sqrt()],
            [4., 0., 7.0_f64.sqrt()],
        ),
    ];
    for &(steps, pick, center, seam) in cases {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        for step in steps {
            enter(&mut app, step);
        }
        let pick = Point3::try_from(pick).unwrap();
        let getter = app.sphere_preview_input().unwrap();
        let preview = getter.preview(pick, app.document.tolerance()).unwrap();
        let expected = NurbsSurface::try_sphere(preview.frame, preview.radius).unwrap();
        assert!(app.document.objects().next().is_none());
        assert!(!app.document.can_undo());
        assert!(
            app.accept_drafting_point(pick),
            "{steps:?}: {:?}",
            app.command_log
        );
        assert_eq!(surface(&app), &expected, "{steps:?}");
        let center = Point3::try_from(center).unwrap();
        let seam = Point3::try_from(seam).unwrap();
        assert!(preview.frame.origin().distance_to(center).unwrap() < 1e-10);
        assert!(
            surface(&app)
                .evaluate(0., 0.)
                .unwrap()
                .distance_to(seam)
                .unwrap()
                < 1e-10,
            "{steps:?}"
        );
        assert_eq!(app.document.undo_label(), Some("Sphere"));
        enter(&mut app, "Undo");
        assert!(app.document.objects().next().is_none());
        enter(&mut app, "Redo");
        assert_eq!(surface(&app), &expected);
    }
}

#[test]
fn sphere_measured_sizes_and_orientation_keep_independent_defaults() {
    let mut app = test_app();
    for (steps, radius) in [
        (&["Sphere", "0,0,0", "Diameter=0.6cm"][..], 3.),
        (&["Circle 10,0,0 7", "Sphere", "20,0,0", ""][..], 3.),
        (
            &["Sphere", "30,0,0", "Orientation", "30,1,1", "4mm"][..],
            2.,
        ),
        (
            &[
                "Sphere",
                "40,0,0",
                "Circumference",
                "w40,0,12.566370614359172",
            ][..],
            2.,
        ),
        (&["Sphere", "50,0,0", "Area=50.26548245743669"][..], 4.),
        (&["Sphere", "60,0,0", "6"][..], 3.),
    ] {
        for step in steps {
            enter(&mut app, step);
        }
        assert!(
            app.active_command.is_none(),
            "{steps:?}: {:?}",
            app.command_log
        );
        let sphere = surface(&app);
        let south = sphere.control_point(0, 0).unwrap().point();
        let north = sphere.control_point(0, 4).unwrap().point();
        assert!((south.distance_to(north).unwrap() * 0.5 - radius).abs() < 1e-10);
    }
    assert!(app.commands.sphere_preferences().diameter);
    assert_eq!(app.commands.circle_preferences().radius, 7.);
}

#[test]
fn sphere_hover_paints_section_then_sphere_and_cancel_preserves_redo() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Sphere");
    enter(&mut app, "0,0");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let radius = screen_point(&app, point(4., 2., 0.));
    let wires = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(radius)]);
    assert_eq!(wires.len(), 192);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, radius);
    assert_eq!(app.document.undo_label(), Some("Sphere"));
    enter(&mut app, "Undo");
    enter(&mut app, "Sphere 4Point");
    enter(&mut app, "3,0");
    enter(&mut app, "0,3");
    let third = screen_point(&app, point(-3., 0., 0.));
    let section = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(third)]);
    assert_eq!(section.len(), 64);
    viewport_click(&mut app, &context, third);
    assert!(app.sphere_preview_input().is_some());
    app.cancel_current_prompt_or_selection();
    assert!(viewport_frame(&mut app, &context, vec![]).is_empty());
    assert!(app.document.can_redo());
}

#[test]
fn sphere_tangent_point_preview_matches_the_native_base_circle() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    for input in ["Sphere", "Tangent", "Point", "3,0", "Point", "0,3", "Point"] {
        enter(&mut app, input);
    }
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let third = screen_point(&app, point(-3., 0., 0.));
    let wires = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(third)]);
    assert!(wires.len() > 64);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, third);
    assert!(
        surface(&app)
            .evaluate(0., 0.)
            .unwrap()
            .distance_to(point(3., 0., 0.))
            .unwrap()
            < 1e-6
    );
    assert_eq!(app.document.undo_label(), Some("Sphere"));
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::NurbsSurface(_)
    ));
}

#[test]
fn sphere_around_curve_previews_native_snapped_radius_and_preserves_the_guide() {
    for preselected in [false, true] {
        let mut app = test_app();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        app.maximized_viewport = Some(0);
        app.osnap = false;
        enter(&mut app, "Line 0,0,0 0,10,10");
        let guide = app.document.objects().next().unwrap().id();
        let guide_geometry = app.document.object(guide).unwrap().geometry().clone();
        enter(&mut app, "Line 6,3,8 9,3,8");
        if preselected {
            app.document
                .select_object(guide, SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "Sphere AroundCurve");
        if !preselected {
            enter(&mut app, &format!("Curve={guide}"));
        }
        enter(&mut app, "w0,3,3");
        enter(&mut app, "_End");
        let context = egui::Context::default();
        viewport_frame(&mut app, &context, vec![]);
        let pointer = screen_point(&app, point(6., 3., 8.));
        let wires = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pointer)]);
        assert_eq!(wires.len(), 192);
        viewport_click(&mut app, &context, pointer);
        let saved_sphere = surface(&app).clone();
        let seam = saved_sphere.evaluate(0., 0.).unwrap();
        assert!(seam.distance_to(point(6., 0.5, 5.5)).unwrap() < 1e-10);
        assert_eq!(
            app.document.object(guide).unwrap().geometry(),
            &guide_geometry
        );
        assert_eq!(app.document.undo_label(), Some("Sphere"));
        assert_eq!(app.document.is_selected(guide), preselected);
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().count(), 2);
        assert_eq!(app.document.is_selected(guide), preselected);
        enter(&mut app, "Redo");
        assert_eq!(surface(&app), &saved_sphere);
        assert_eq!(app.document.is_selected(guide), preselected);
    }
}
