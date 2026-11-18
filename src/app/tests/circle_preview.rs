use super::{curve_fillet::enter, *};

fn final_circle(app: &VibocerosApp) -> viboceros_geometry::Circle3 {
    let Geometry::Circle(circle) = app.document.objects().last().unwrap().geometry() else {
        panic!("expected an analytic circle");
    };
    *circle
}

#[test]
fn circle_previews_match_accepted_geometry_across_point_and_size_modes() {
    let cases: &[&[&str]] = &[
        &["Circle", "w1,2,3"],
        &["Circle", "w1,2,3", "Diameter"],
        &["Circle", "w1,2,3", "Circumference"],
        &["Circle", "w1,2,3", "Area"],
        &["Circle", "w1,2,3", "Orientation", "w3,3,5"],
        &["Circle", "w1,2,3", "Orientation", "w3,3,5", "Area"],
        &["Circle Vertical", "w1,2,3"],
        &["Circle Vertical", "w1,2,3", "8"],
        &["Circle Vertical", "w1,2,3", "Area"],
        &["Circle Vertical", "w1,2,3", "Circumference"],
        &["Circle 2Point", "w1,2,3"],
        &["Circle Vertical 2Point", "w1,2,3"],
        &["Circle 2Point", "Vertical", "w1,2,3"],
        &["Circle Vertical", "2Point", "w1,2,3"],
        &["Circle 3Point", "w1,2,3", "w6,3,1"],
        &["Circle 3Point", "w1,2,3", "w6,3,1", "Radius"],
        &["Circle 3Point", "w1,2,3", "w6,3,1", "Radius=9"],
        &["Circle 3Point", "w1,2,3", "w6,3,1", "Radius 0.9cm"],
    ];
    for &inputs in cases {
        let mut app = test_app();
        for input in inputs {
            enter(&mut app, input);
        }
        let (construction, plane) = app
            .circle_preview()
            .unwrap_or_else(|| panic!("no preview for {inputs:?}: {:?}", app.command_log));
        let target = point(5., 6., 7.);
        let preview = construction
            .circle(target, plane, app.document.tolerance())
            .unwrap();
        assert!(app.document.objects().next().is_none());
        assert!(!app.document.can_undo());

        // Leave the original viewport after the first pick, then finish with a
        // world point. Preview and acceptance both use the captured CPlane.
        app.active_viewport = 1;
        assert_eq!(app.circle_preview().unwrap().1, plane);
        enter(&mut app, "w5,6,7");
        assert_eq!(preview, final_circle(&app), "{inputs:?}");
        assert!(app.circle_preview().is_none());
        assert_eq!(app.document.undo_label(), Some("Circle"));
    }
}

#[test]
fn circle_remembers_radius_and_diameter_and_accepts_typed_units() {
    let mut app = test_app();
    for (inputs, radius) in [
        (&["Circle", "0,0", "0.4cm"][..], 4.),
        (&["Circle", "12,0", ""][..], 4.),
        (&["Circle", "24,0", "Diameter 6mm"][..], 3.),
        (&["Circle", "36,0", ""][..], 3.),
        (&["Circle", "48,0", "8"][..], 4.),
        (&["Circle", "60,0", "Orientation", "60,1,1", ""][..], 4.),
        (&["Circle Vertical", "72,0", "", "75,4,1"][..], 4.),
        (
            &["Circle 3Point", "1,0", "0,1", "Radius", "", "2,3"][..],
            4.,
        ),
        (
            &["Circle 3Point", "1,0", "0,1", "Radius", "3", "2,3"][..],
            3.,
        ),
        (&["Circle", "80,0", "6"][..], 3.),
        (
            &["Circle Vertical", "84,0", "Area=12.566370614359172"][..],
            2.,
        ),
    ] {
        for input in inputs {
            enter(&mut app, input);
        }
        assert!(
            app.active_command.is_none(),
            "{inputs:?}: {:?}",
            app.command_log
        );
        assert!(
            (final_circle(&app).radius() - radius).abs() < 1e-12,
            "{inputs:?}"
        );
    }
}

pub(super) fn viewport_frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<[egui::Pos2; 2]> {
    let output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000., 800.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show_frame(ui, None),
    );
    let preview: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match shape.shape {
            egui::Shape::LineSegment { points, stroke }
                if stroke.color == egui::Color32::from_rgb(20, 115, 190) =>
            {
                Some(points)
            }
            _ => None,
        })
        .collect();
    output.drop_without_applying_deltas();
    preview
}

pub(super) fn screen_point(app: &VibocerosApp, point: Point3) -> egui::Pos2 {
    let view = &app.viewports[0];
    view.project(point, view.screen_rect().unwrap()).unwrap()
}

pub(super) fn viewport_click(app: &mut VibocerosApp, context: &egui::Context, pos: egui::Pos2) {
    for pressed in [true, false] {
        viewport_frame(
            app,
            context,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
}

#[test]
fn circle_hover_draws_current_geometry_and_click_commits_it() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Circle");
    enter(&mut app, "0,0,0");
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events| viewport_frame(app, &context, events);
    assert!(frame(&mut app, vec![]).is_empty());
    let first = screen_point(&app, point(2., 0., 0.));
    let first_preview = frame(&mut app, vec![egui::Event::PointerMoved(first)]);
    assert_eq!(first_preview.len(), 64);
    let target = screen_point(&app, point(4., 0., 0.));
    let preview = frame(&mut app, vec![egui::Event::PointerMoved(target)]);
    assert_eq!(preview.len(), 64);
    assert_ne!(preview, first_preview);
    assert!(!app.document.can_undo());
    assert!(app.document.objects().next().is_none());
    viewport_click(&mut app, &context, target);
    assert!((final_circle(&app).radius() - 4.).abs() < 1e-4);
    assert!(frame(&mut app, vec![]).is_empty());
    enter(&mut app, "Undo");
    enter(&mut app, "Circle Vertical 2Point");
    enter(&mut app, "0,0");
    let target = screen_point(&app, point(3., 4., 0.));
    assert!(!frame(&mut app, vec![egui::Event::PointerMoved(target)]).is_empty());
    app.cancel_current_prompt_or_selection();
    assert!(frame(&mut app, vec![]).is_empty());
    assert!(app.document.can_redo());
}

#[test]
fn circle_projects_only_object_snaps_and_remembers_the_option() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    enter(&mut app, "Line 6,0,8 9,0,8");
    let context = egui::Context::default();
    for (command, option, mode, z) in [
        ("Circle", None, None, 3.),
        ("Circle", Some("ProjectOsnap=No"), None, 8.),
        ("Circle Vertical", None, None, 8.),
        ("Circle Vertical", Some("_ProjectOsnap"), None, 3.),
        ("Circle", None, Some("Area"), 8.),
        ("Circle", None, Some("Circumference"), 8.),
    ] {
        enter(&mut app, command);
        enter(&mut app, "w0,0,3");
        if let Some(option) = option {
            enter(&mut app, option);
        }
        if let Some(mode) = mode {
            enter(&mut app, mode);
        }
        enter(&mut app, "_End");
        viewport_frame(&mut app, &context, vec![]);
        let target = screen_point(&app, point(6., 0., 8.));
        let wires = viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
        let (construction, plane) = app.circle_preview().unwrap();
        let expected = construction
            .circle(point(6., 0., z), plane, app.document.tolerance())
            .unwrap();
        let check_pixel = screen_point(
            &app,
            expected
                .point_at_angle(std::f64::consts::FRAC_PI_2)
                .unwrap(),
        );
        viewport_click(&mut app, &context, target);
        let actual = final_circle(&app);
        assert_eq!(actual, expected, "{command}, {option:?}, {mode:?}");
        assert!(
            wires
                .iter()
                .flatten()
                .any(|&p| p.distance(check_pixel) < 1e-3),
            "{command}, {option:?}, {mode:?}: did not draw {check_pixel:?} in {} segments",
            wires.len(),
        );
        assert!(app.snaps.model_override.is_none());
        enter(&mut app, "Undo");
    }

    enter(&mut app, "Circle Deformable");
    enter(&mut app, "w0,0,3");
    assert!(
        app.viewport_drafting_input(false, false)
            .snap_projection
            .is_some()
    );
    enter(&mut app, "ProjectOsnap=No");
    assert!(
        app.viewport_drafting_input(false, false)
            .snap_projection
            .is_none()
    );
    enter(&mut app, "ProjectOsnap=Yes");
    enter(&mut app, "_End");
    viewport_frame(&mut app, &context, vec![]);
    let target = screen_point(&app, point(6., 0., 8.));
    assert!(
        !viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]).is_empty()
    );
    let expected = app
        .circle_deform_session
        .as_ref()
        .unwrap()
        .input
        .preview(point(6., 0., 3.))
        .unwrap();
    viewport_click(&mut app, &context, target);
    assert_eq!(
        app.document.objects().last().unwrap().geometry(),
        &Geometry::NurbsCurve(expected)
    );

    enter(&mut app, "Circle");
    enter(&mut app, "w0,0,3");
    enter(&mut app, "w6,0,8");
    assert!((final_circle(&app).radius() - 61.0_f64.sqrt()).abs() < 1e-12);
}

#[test]
fn circle_around_curve_project_option_uses_the_guide_plane() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    app.osnap = false;
    enter(&mut app, "Line 0,0,0 0,10,10");
    let guide = app.document.objects().next().unwrap().id();
    enter(&mut app, "Line 6,3,8 9,3,8");
    let context = egui::Context::default();
    for project in [true, false] {
        enter(&mut app, "Circle AroundCurve");
        enter(&mut app, &format!("Curve={guide}"));
        enter(&mut app, "w0,3,3");
        let option = if project { "Yes" } else { "No" };
        enter(&mut app, &format!("ProjectOsnap={option}"));
        assert_eq!(
            app.viewport_drafting_input(false, false)
                .snap_projection
                .is_some(),
            project
        );
        enter(&mut app, "_End");
        viewport_frame(&mut app, &context, vec![]);
        let target = screen_point(&app, point(6., 3., 8.));
        assert!(
            !viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]).is_empty()
        );
        viewport_click(&mut app, &context, target);
        let circle = final_circle(&app);
        assert!((circle.radius() - (48.5_f64).sqrt()).abs() < 1e-12);
        assert!(
            circle
                .point_at_angle(0.)
                .unwrap()
                .distance_to(point(6., 0.5, 5.5))
                .unwrap()
                < 1e-12
        );
        enter(&mut app, "Undo");
    }
}

#[test]
fn transparent_zoom_target_uses_the_original_circle_snap() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Line 6,0,8 9,0,8");
    enter(&mut app, "Circle");
    enter(&mut app, "w0,0,3");
    enter(&mut app, "_End");
    assert!(
        app.viewport_drafting_input(false, false)
            .snap_projection
            .is_some()
    );
    enter(&mut app, "ZT");
    let context = egui::Context::default();
    viewport_frame(&mut app, &context, vec![]);
    let endpoint = point(6., 0., 8.);
    let pointer = screen_point(&app, endpoint);
    viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(pointer)]);
    viewport_click(&mut app, &context, pointer);
    assert!(matches!(
        app.zoom_target,
        Some(ZoomTargetState::PickWindow {
            target,
            viewport: 0
        }) if target == endpoint
    ));
    assert!(app.circle_preview().is_some());
}
