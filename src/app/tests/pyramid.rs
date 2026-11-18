use super::{
    circle_preview::{screen_point, viewport_click, viewport_frame},
    curve_fillet::enter,
    *,
};

#[test]
fn pyramid_mouse_updates_the_base_and_apex_preview_before_one_edit() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.grid_snap = false;
    app.osnap = false;
    enter(&mut app, "Pyramid NumSides=5");
    enter(&mut app, "w1,2,0");
    let ui = egui::Context::default();
    viewport_frame(&mut app, &ui, vec![]);
    viewport_frame(&mut app, &ui, vec![]);
    let base_corner = screen_point(&app, point(4., 2., 0.));
    let shape = viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(base_corner)]);
    assert_eq!(shape.len(), 5);
    assert_eq!(app.document.objects().len(), 0);
    viewport_click(&mut app, &ui, base_corner);
    assert!(
        app.pyramid_session
            .as_ref()
            .unwrap()
            .input
            .prompt()
            .starts_with("End of pyramid")
    );

    app.viewports[0] = Viewport::new(ViewKind::Front);
    viewport_frame(&mut app, &ui, vec![]);
    viewport_frame(&mut app, &ui, vec![]);
    let target = screen_point(&app, point(1., 2., 6.));
    let first = screen_point(&app, point(1., 2., 4.));
    let before = viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(first)]);
    let after = viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(target)]);
    assert!(after.len() >= 9);
    assert_ne!(before, after);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &ui, target);
    let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected a closed pyramid");
    };
    assert!(brep.is_solid());
    assert!(
        brep.vertices()[2]
            .point()
            .distance_to(point(1., 2., 6.))
            .unwrap()
            < 2e-5
    );
    assert_eq!(brep.faces().len(), 6);
    assert!(app.pyramid_session.is_none());
    assert!(viewport_frame(&mut app, &ui, vec![]).is_empty());
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pyramid"));
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.document.redo().unwrap().as_deref(), Some("Pyramid"));
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn pyramid_edge_and_star_options_preserve_valid_input_and_reset_the_base_mode() {
    let mut app = test_app();
    for token in [
        "Pyramid", "NumSides", "5", "Solid=No", "Edge", "w1,2,3", "Flip", "w1,2,3",
    ] {
        enter(&mut app, token);
    }
    let input = app.pyramid_session.as_ref().unwrap().input;
    assert!(
        input.prompt().starts_with("End of edge"),
        "{:?}",
        app.command_log
    );
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "w5,2,3");
    assert!(
        app.pyramid_session
            .as_ref()
            .unwrap()
            .input
            .preview(None)
            .is_some()
    );
    app.cancel_current_prompt_or_selection();
    assert!(!app.document.can_undo());
    enter(&mut app, "Pyramid");
    let new = app.pyramid_session.as_ref().unwrap().input;
    assert!(new.prompt().starts_with("Center of inscribed pyramid"));
    assert!(!new.preferences().solid);
    for token in ["Star", "w1,2,3", "w5,2,3", "Enter", "w1,2,8"] {
        enter(&mut app, token);
    }
    let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected star pyramid");
    };
    assert_eq!(brep.faces().len(), 10);
    assert!(!brep.is_solid());
    assert!(
        brep.vertices()[1]
            .point()
            .distance_to(point(2.23606797749979, 2.8980559531591705, 3.))
            .unwrap()
            < 1e-12
    );
    assert!(app.pyramid_session.is_none());
}

#[test]
fn pyramid_around_curve_reuses_the_curve_picker_and_keeps_the_source() {
    let mut app = test_app();
    enter(&mut app, "Line w2,3,4 w6,7,12");
    let guide = app.document.objects().next().unwrap().id();
    let original = app
        .document
        .object(guide)
        .unwrap()
        .geometry_snapshot()
        .clone();
    enter(&mut app, "Pyramid DirectionConstraint=AroundCurve");
    assert!(app.picking_circle_around_curve());
    assert!(app.pick_circle_around_curve(Some(guide), None));
    assert!(app.document.is_selected(guide));
    enter(&mut app, "w4,5,8");
    assert!(app.circle_around_session.is_none());
    assert!(!app.document.is_selected(guide));
    let input = app.pyramid_session.as_ref().unwrap().input;
    assert_eq!(input.anchor(), Some(point(4., 5., 8.)));
    assert!(input.mouse_plane().is_some());
    enter(&mut app, "w6,2,8");
    enter(&mut app, "-5");
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected guide-backed pyramid");
    };
    let apex = point(1.9587585476806852, 2.9587585476806844, 3.9175170953613696);
    assert!(brep.vertices()[2].point().distance_to(apex).unwrap() < 1e-12);
    assert!(original.shares_storage_with(app.document.object(guide).unwrap().geometry_snapshot()));
    assert_eq!(app.document.undo().unwrap().as_deref(), Some("Pyramid"));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.is_selected(guide));
}

#[test]
fn pyramid_free_star_uses_the_cursor_at_the_second_radius() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.grid_snap = false;
    app.osnap = false;
    for token in ["Pyramid DirectionConstraint=None Star", "w0,0,0", "w4,0,0"] {
        enter(&mut app, token);
    }
    let ui = egui::Context::default();
    viewport_frame(&mut app, &ui, vec![]);
    viewport_frame(&mut app, &ui, vec![]);
    // A proportionally nearer cursor with the same azimuth as the native ray.
    let cursor = screen_point(&app, point(-1.237368292662076, -3.516910878421608, 0.));
    assert_eq!(
        viewport_frame(&mut app, &ui, vec![egui::Event::PointerMoved(cursor)]).len(),
        10
    );
    enter(&mut app, "2");
    let apex = point(5., 5., 6.);
    let input = app.pyramid_session.as_ref().unwrap().input;
    let (base, predicted_apex) = input.preview(Some(apex)).unwrap();
    assert_eq!(predicted_apex, Some(apex));
    let first = base.rim().unwrap()[0];
    let native = point(-0.28253514616289466, 3.1778520213057218, -2.412764062619023);
    assert!(first.distance_to(native).unwrap() < 2e-5);

    // The final height pick uses the center as the relative coordinate origin.
    enter(&mut app, "r5,5,6");
    let Geometry::Brep(brep) = app.document.objects().next().unwrap().geometry() else {
        panic!("expected a free-direction star pyramid");
    };
    assert_eq!(brep.vertices()[0].point(), first);
    assert_eq!(brep.vertices()[2].point(), apex);
    assert_eq!(brep.faces().len(), 11);
}

#[test]
fn pyramid_edge_exits_a_remembered_curve_constraint() {
    let mut app = test_app();
    enter(&mut app, "Pyramid DirectionConstraint=AroundCurve");
    assert!(app.picking_circle_around_curve());
    enter(&mut app, "Edge");
    assert!(app.circle_around_session.is_none());
    let input = app.pyramid_session.as_ref().unwrap().input;
    assert!(input.prompt().starts_with("Start of edge"));
    assert!(input.prompt().contains("DirectionConstraint=Vertical"));
}
