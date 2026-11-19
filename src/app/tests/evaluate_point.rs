use super::{circle_preview::screen_point, circle_preview::viewport_click, curve_fillet::enter, *};

#[test]
fn evaluate_point_uses_pick_view_plane_and_preserves_selection_and_history() {
    let mut app = test_app();
    app.execute_command("Point 1,2,3");
    app.execute_command("SelAll");
    app.execute_command("Point 4,5,6");
    app.execute_command("Undo");
    let before = format!("{:?}", app.document);
    let frame = Frame3::try_from_directions(
        point(10., 20., 30.),
        viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[app.active_viewport].plane.set(frame);
    for input in ["EvaluatePt", "_EvaluatePt _Label=_Off"] {
        assert!(app.try_start_interactive_command(input));
        assert!(app.try_continue_point_input("4,5,3"));
        assert!(app.active_command.is_none());
        assert_eq!(
            app.command_log.back().unwrap(),
            "World coordinates = 13,24,35\nCPlane coordinates = 4,5,3"
        );
        assert_eq!(format!("{:?}", app.document), before);
    }
    assert!(app.try_start_interactive_command("EvaluatePt"));
    app.cancel_interactive_command(true);
    assert_eq!(format!("{:?}", app.document), before);
    app.execute_command("Redo");
    assert_eq!(app.document.objects().count(), 2);
    assert!(!app.try_start_interactive_command("EvaluatePt Label=No"));
    assert!(!app.try_start_interactive_command("EvaluatePt 1,2,3"));
}

#[test]
fn evaluate_point_retains_prompt_after_unrepresentable_local_coordinate() {
    let mut app = test_app();
    let plane = app.viewports[app.active_viewport]
        .construction_plane()
        .with_origin(point(-1e308, 0., 0.));
    app.viewports[app.active_viewport].plane.set(plane);
    let before = format!("{:?}", app.document);
    let previous_last = app.last_point;
    assert!(app.try_start_interactive_command("EvaluatePt"));
    assert!(!app.accept_drafting_point(point(1e308, 0., 0.)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::EvaluatePoint { .. })
    ));
    assert_eq!(app.last_point, previous_last);
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert!(app.command_log.back().unwrap().contains("1e308"));
}

#[test]
fn evaluate_point_changes_label_options_while_getting_a_typed_point() {
    let mut app = test_app();
    let frame = Frame3::try_from_directions(
        point(10., 20., 30.),
        viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[app.active_viewport].plane.set(frame);
    for input in [
        "_EvaluatePt _Label=_On",
        "Label",
        "Label On",
        "_CoordinateSystem",
        "_CPlane",
        "Style",
        "Dot",
    ] {
        enter(&mut app, input);
        assert!(app.document.objects().next().is_none());
        assert!(!app.document.can_undo());
    }
    enter(&mut app, "4,5,3");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let object = app.document.objects().next().unwrap();
    let Geometry::TextDot(dot) = object.geometry() else {
        panic!("expected a text dot");
    };
    assert_eq!(dot.point(), point(13., 24., 35.));
    assert_eq!(dot.text(), "4.00,5.00,3.00");
    assert_eq!(
        app.command_log.back().unwrap(),
        "World coordinates = 13,24,35\nCPlane coordinates = 4,5,3"
    );
    assert_eq!(app.document.undo_label(), Some("EvaluatePt"));
    let id = object.id();
    enter(&mut app, "Undo");
    assert!(app.document.objects().next().is_none());
    enter(&mut app, "Redo");
    assert!(app.document.object(id).is_some());
}

#[test]
fn evaluate_point_label_is_drawn_before_click_and_selectable_after_placement() {
    let mut app = test_app();
    let style_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/viboceros-io/tests/fixtures/evaluate-point-annotation-style.3dm");
    let style = viboceros_io::read_3dm_file(style_file, Tolerance::DEFAULT)
        .unwrap()
        .annotation_style;
    app.document.set_annotation_style(style);
    app.viewports[0] = Viewport::new(ViewKind::Top);
    let plane = app.viewports[0]
        .construction_plane()
        .with_origin(point(1., 1., 0.));
    app.viewports[0].plane.set(plane);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events| {
        let mut output = context.run_ui(
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
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();
        shapes
    };
    enter(
        &mut app,
        "EvaluatePt Label=On CoordinateSystem=CPlane Style=Dot",
    );
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::EvaluatePoint { options, pending: None }) if options.label
    ));
    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    let anchor = point(4., 2., 0.);
    let pixel = screen_point(&app, anchor);
    let hover = frame(&mut app, vec![egui::Event::PointerMoved(pixel)]);
    let labels: Vec<_> = hover
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    let expected = "0.235,0.080,0.000";
    let preview = labels
        .iter()
        .find(|label| label.galley.job.text == expected)
        .unwrap_or_else(|| {
            panic!(
                "missing drawn CPlane label among {:?}",
                labels
                    .iter()
                    .map(|label| &label.galley.job.text)
                    .collect::<Vec<_>>()
            )
        });
    assert!(app.document.objects().next().is_none());
    viewport_click(&mut app, &context, pixel);
    let object = app.document.objects().next().unwrap();
    let id = object.id();
    let Geometry::TextDot(dot) = object.geometry() else {
        panic!("expected a text dot");
    };
    assert_eq!(dot.point(), anchor);
    assert_eq!(dot.text(), preview.galley.job.text);

    // Use the final layout because the command panel can resize on completion.
    frame(&mut app, vec![]);
    let placed = frame(&mut app, vec![]);
    let placed_label = placed
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == expected => Some(text),
            _ => None,
        })
        .expect("drawn model label");

    // Select through the label text, away from the model anchor.
    let inside_right_edge = placed_label.pos
        + egui::vec2(
            placed_label.galley.size().x - 1.,
            placed_label.galley.size().y * 0.5,
        );
    viewport_click(&mut app, &context, inside_right_edge);
    assert!(app.document.is_selected(id));
}
