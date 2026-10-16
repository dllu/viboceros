use super::*;
#[test]
fn extract_curvature_graph_viewport_click_filters_coincident_ungraphed_candidates() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    let mut doc = Document::default();
    for _ in 0..2 {
        registry.execute(&mut doc, "Line -2,0,0 2,0,0").unwrap();
    }
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    let allowed = std::collections::BTreeSet::from([ids[1]]);
    let context = egui::Context::default();
    let mut viewport = Viewport::new(ViewKind::Top);
    let mut frame = |events| {
        let mut result = None;
        let output = context.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                ..Default::default()
            },
            |ui| {
                result = Some(viewport.show(
                    ui,
                    &doc,
                    ViewportInput {
                        selection_allowed_objects: Some(&allowed),
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                ));
            },
        );
        output.drop_without_applying_deltas();
        result.unwrap()
    };
    let pointer = Pos2::new(400., 300.);
    let click = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![]);
    frame(vec![egui::Event::PointerMoved(pointer), click(true)]);
    let result = frame(vec![click(false)]);
    assert!(result.selection_choice.is_none());
    assert_eq!(result.selection_click.unwrap().object_id, Some(ids[1]));
}
#[test]
fn curvature_graph_overlay_is_visible_in_every_mode_and_respects_hidden_sources() {
    let registry = viboceros_command::CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Circle 0,0,0 2").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(&mut doc, "CurvatureGraph Color=3,211,137")
        .unwrap();
    let view = registry.curvature_graph_view(&doc).unwrap().unwrap();
    let before = format!("{doc:?}");
    for kind in [
        ViewKind::Top,
        ViewKind::Perspective,
        ViewKind::Front,
        ViewKind::Right,
    ] {
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut counts = Vec::new();
            for overlay in [None, Some(&view)] {
                let context = egui::Context::default();
                let mut viewport = Viewport::new(kind);
                viewport.display_mode = mode;
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                        ..Default::default()
                    },
                    |ui| {
                        viewport.show(
                            ui,
                            &doc,
                            ViewportInput {
                                curvature_graph: overlay,
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                );
                counts.push(output.shapes.iter().filter(|s|matches!(&s.shape,egui::Shape::LineSegment{stroke,..}if stroke.color==Color32::from_rgb(3,211,137))).count());
                output.drop_without_applying_deltas();
            }
            assert_eq!(counts[0], 0);
            assert!(counts[1] > 0, "{kind:?} {mode:?}");
        }
    }
    assert_eq!(format!("{doc:?}"), before);
    registry.execute(&mut doc, "Hide").unwrap();
    let context = egui::Context::default();
    let viewport = Viewport::new(ViewKind::Top);
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        viewport.paint_curvature_graph(
            ui.painter(),
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(800.)),
            &doc,
            &view,
        );
    });
    assert!(
        !output
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::LineSegment { .. }))
    );
    output.drop_without_applying_deltas();
}
