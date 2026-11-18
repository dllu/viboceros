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

#[test]
fn circle_hover_draws_current_geometry_and_click_commits_it() {
    let mut app = test_app();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Circle");
    enter(&mut app, "0,0,0");
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events| {
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
    };
    assert!(frame(&mut app, vec![]).is_empty());
    let pixel = |app: &VibocerosApp, point| {
        let view = &app.viewports[0];
        view.project(point, view.screen_rect().unwrap()).unwrap()
    };
    let first = pixel(&app, point(2., 0., 0.));
    let first_preview = frame(&mut app, vec![egui::Event::PointerMoved(first)]);
    assert_eq!(first_preview.len(), 64);
    let target = pixel(&app, point(4., 0., 0.));
    let preview = frame(&mut app, vec![egui::Event::PointerMoved(target)]);
    assert_eq!(preview.len(), 64);
    assert_ne!(preview, first_preview);
    assert!(!app.document.can_undo());
    assert!(app.document.objects().next().is_none());
    for pressed in [true, false] {
        frame(
            &mut app,
            vec![egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    assert!((final_circle(&app).radius() - 4.).abs() < 1e-4);
    assert!(frame(&mut app, vec![]).is_empty());
    enter(&mut app, "Undo");
    enter(&mut app, "Circle Vertical 2Point");
    enter(&mut app, "0,0");
    let target = pixel(&app, point(3., 4., 0.));
    assert!(!frame(&mut app, vec![egui::Event::PointerMoved(target)]).is_empty());
    app.cancel_current_prompt_or_selection();
    assert!(frame(&mut app, vec![]).is_empty());
    assert!(app.document.can_redo());
}
