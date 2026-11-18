use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};
use viboceros_command::torus_input::Kind;
use viboceros_geometry::NurbsSurface;

fn surface(app: &VibocerosApp) -> &NurbsSurface {
    let Geometry::NurbsSurface(surface) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected a torus surface");
    };
    surface
}

#[test]
fn solid_torus_previews_base_and_exact_surface_sections_before_commit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Torus 2Point");
    enter(&mut app, "w1,2,3");
    let diameter_end = point(5., 8., 9.);
    app.update_torus_hover(diameter_end);
    assert_eq!(
        app.torus_session.as_ref().unwrap().preview_curves().len(),
        1
    );
    assert!(app.document.objects().next().is_none());
    assert!(app.accept_drafting_point(diameter_end));
    let session = app.torus_session.as_ref().unwrap();
    assert_eq!(session.input.kind, Kind::Solid);
    let base = session.input.circle.unwrap();
    enter(&mut app, "10");
    assert!(app.document.objects().next().is_none());
    assert_eq!(app.torus_session.as_ref().unwrap().input.circle, Some(base));

    let tube_point = base.frame.point_at([base.radius + 1.25, 0., 7.]).unwrap();
    app.update_torus_hover(tube_point);
    let preview = app
        .torus_session
        .as_ref()
        .unwrap()
        .preview_curves()
        .to_vec();
    assert_eq!(preview.len(), 8);
    let wires = viewport_frame(&mut app, &egui::Context::default(), vec![]);
    assert!(!wires.is_empty());
    assert!(app.document.undo_label().is_none());
    assert!(app.accept_drafting_point(tube_point));

    let saved = surface(&app).clone();
    let u_end = *saved.domain_u().end();
    let v_end = *saved.domain_v().end();
    for (index, f) in [0., 0.25, 0.5, 0.75].into_iter().enumerate() {
        assert_eq!(preview[index * 2], saved.isocurve_u(f * v_end).unwrap());
        assert_eq!(preview[index * 2 + 1], saved.isocurve_v(f * u_end).unwrap());
    }
    assert_eq!(app.document.undo_label(), Some("Torus"));
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert_eq!(&saved, surface(&app));
}

#[test]
fn solid_torus_remembers_base_and_tube_options_after_cancellation() {
    let mut app = test_app();
    assert_eq!(app.commands.torus_preferences().tube_radius, 1.);
    assert!(!app.commands.torus_preferences().tube_diameter);
    assert!(app.commands.mesh_torus_preferences().tube_diameter);
    for step in [
        "Torus",
        "0,0",
        "Diameter=8",
        "FixInnerDimension=Yes",
        "Diameter=4",
    ] {
        enter(&mut app, step);
    }
    let saved = surface(&app).clone();
    assert_eq!(
        saved.control_point(0, 0).unwrap().point(),
        point(8., 0., 0.)
    );
    assert_eq!(*saved.domain_v().end(), 2. * std::f64::consts::TAU);
    enter(&mut app, "Undo");
    for step in [
        "Torus",
        "0,0",
        "Diameter=18",
        "FixInnerDimension=No",
        "Radius",
        "Cancel",
    ] {
        enter(&mut app, step);
    }
    assert!(app.torus_session.is_none());
    let remembered = app.commands.torus_preferences();
    assert_eq!(remembered.radius, 9.);
    assert_eq!(remembered.tube_radius, 2.);
    assert!(!remembered.tube_diameter);
    assert!(!remembered.fix_inner_dimension);
    assert_eq!(app.document.redo_label(), Some("Torus"));
    for step in ["Torus", "0,0", "Enter", "Enter"] {
        enter(&mut app, step);
    }
    let repeated = surface(&app);
    assert_eq!(
        repeated.control_point(0, 0).unwrap().point(),
        point(11., 0., 0.)
    );
    assert_eq!(*repeated.domain_v().end(), *saved.domain_v().end());
}

#[test]
fn solid_torus_remembers_vertical_size_locks_and_base_display_mode() {
    let mut app = test_app();
    for step in ["Torus", "0,0", "Diameter", "Cancel"] {
        enter(&mut app, step);
    }
    assert!(app.commands.torus_preferences().diameter);
    for step in ["Torus Vertical", "0,0", "7", "Cancel"] {
        enter(&mut app, step);
    }
    assert_eq!(app.commands.torus_preferences().radius, 3.5);
    for step in ["Torus 3Point", "-2,0", "2,0", "Radius", "3", "Cancel"] {
        enter(&mut app, step);
    }
    assert_eq!(app.commands.torus_preferences().radius, 3.5);
    assert!(app.document.undo_label().is_none());
    for step in ["Torus", "0,0", "Enter", "1"] {
        enter(&mut app, step);
    }
    assert_eq!(
        surface(&app).control_point(0, 0).unwrap().point(),
        point(4.5, 0., 0.)
    );
}

#[test]
fn solid_torus_reference_modes_resume_the_tube_stage_and_release_command_picks() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    for step in [
        "Torus Tangent",
        "Point",
        "4,0",
        "Point",
        "0,4",
        "Point",
        "-4,0",
    ] {
        enter(&mut app, step);
    }
    assert_eq!(
        app.torus_session
            .as_ref()
            .unwrap()
            .input
            .circle
            .unwrap()
            .radius,
        4.
    );
    enter(&mut app, "1");
    assert_eq!(
        surface(&app).control_point(0, 0).unwrap().point(),
        point(5., 0., 0.)
    );
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().last().unwrap().id();
    let geometry = app.document.object(guide).unwrap().geometry().clone();
    for step in [
        "Torus AroundCurve",
        &format!("Curve={guide}"),
        "w0,3,3",
        "w6,3,8",
        "1",
    ] {
        enter(&mut app, step);
    }
    let radius = 48.5_f64.sqrt();
    assert!((*surface(&app).domain_u().end() - std::f64::consts::TAU * radius).abs() < 1e-10);
    assert_eq!(app.document.object(guide).unwrap().geometry(), &geometry);
    assert!(!app.document.is_selected(guide));

    for p in [point(3., 0., 2.), point(0., 3., 2.), point(-3., 0., 2.)] {
        app.document.add_geometry(Geometry::Point(p)).unwrap();
    }
    enter(&mut app, "Torus FitPoints");
    assert_eq!(
        app.object_prompt.as_ref().unwrap().description.command,
        "Torus FitPoints"
    );
    enter(&mut app, "SelAll");
    assert!(app.torus_fit_preview().is_some());
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    enter(&mut app, "1");
    assert_eq!(app.document.undo_label(), Some("Torus"));
    assert_eq!(app.document.selected_object_count(), 0);
    assert!((app.commands.torus_preferences().radius - radius).abs() < 1e-10);
    enter(&mut app, "Undo");
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn solid_torus_projects_endpoint_snaps_at_the_base_radius_prompt() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    enter(&mut app, "Line 6,0,8 9,0,8");
    let ctx = egui::Context::default();
    for (setting, radius) in [("Yes", 6.), ("No", 61_f64.sqrt())] {
        for step in [
            "Torus",
            "w0,0,3",
            &format!("ProjectOsnap={setting}"),
            "_End",
        ] {
            enter(&mut app, step);
        }
        viewport_frame(&mut app, &ctx, vec![]);
        let pointer = screen_point(&app, point(6., 0., 8.));
        viewport_click(&mut app, &ctx, pointer);
        let base = app.torus_session.as_ref().unwrap().input.circle.unwrap();
        assert!((base.radius - radius).abs() < 1e-10);
        enter(&mut app, "1");
        assert!((*surface(&app).domain_u().end() - std::f64::consts::TAU * radius).abs() < 1e-10);
        enter(&mut app, "Undo");
    }
}
