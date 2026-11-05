use super::*;

#[test]
fn subd_diagnostic_overlays_paint_in_all_views_modes_and_appearances_without_model_objects() {
    let mut doc = Document::default();
    let commands = viboceros_command::CommandRegistry::with_builtins();
    let id = doc
        .add_geometry(
            viboceros_oracle::subd_direction::source("open_cube", doc.tolerance()).unwrap(),
        )
        .unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    commands
        .execute(&mut doc, "ShowEdges Show=Naked Color=10,20,30")
        .unwrap();
    assert!(!doc.is_selected(id));
    for flat in [false, true] {
        if flat {
            commands.execute(&mut doc, "SubDDisplayToggle").unwrap();
        }
        let edges = commands.edge_analysis_view(&doc).unwrap().unwrap();
        assert_eq!(edges.displayed().count(), 4);
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
                let mut view = Viewport::new(kind);
                view.display_mode = mode;
                let context = egui::Context::default();
                let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
                view.last_rect = Some(rect);
                view.zoom_extents(&doc, Default::default()).unwrap();
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        ..Default::default()
                    },
                    |ui| view.paint_edge_analysis(ui.painter(), rect, &doc, &edges),
                );
                let painted=output.shapes.iter().any(|s|matches!(&s.shape,egui::epaint::Shape::LineSegment {stroke,..} if stroke.color==Color32::from_rgb(10,20,30)));
                output.drop_without_applying_deltas();
                assert!(painted, "{kind:?} {mode:?} flat={flat}");
            }
        }
        assert_eq!(format!("{doc:?}"), before);
    }
    doc.set_objects_visibility([id], false).unwrap();
    let edges = commands.edge_analysis_view(&doc).unwrap().unwrap();
    let context = egui::Context::default();
    let view = Viewport::new(ViewKind::Top);
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |ui| view.paint_edge_analysis(ui.painter(), rect, &doc, &edges),
    );
    let empty = output.shapes.is_empty();
    output.drop_without_applying_deltas();
    assert!(empty);
    assert_eq!(doc.objects().len(), 1);
}
