use super::*;
#[test]
fn offset_subd_viewport_clicks_flip_inputs_with_cage_preview_in_each_display_mode() {
    use crate::viewport::{DisplayMode, ViewKind, Viewport, ViewportInput, ViewportOutput};
    use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};
    let sources = support::sources();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        let (doc, id) = support::fixture(&sources["quad"]);
        let mut app = test_app();
        app.document = doc;
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "OffsetSubD");
        let context = Context::default();
        let mut view = Viewport::new(ViewKind::Top);
        view.display_mode = mode;
        let mut time = 0.;
        let mut frame = |events: Vec<Event>, app: &VibocerosApp| {
            time += 0.25;
            let mut output = ViewportOutput::default();
            let mut area = Rect::NOTHING;
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        area = ui.available_rect_before_wrap();
                        output = view.show(
                            ui,
                            &app.document,
                            ViewportInput {
                                object_filter: Some(viboceros_command::ObjectSelectionFilter::SubD),
                                offset_subd_edges: Some(
                                    app.offset_subd_session.as_ref().unwrap().edges(),
                                ),
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                )
                .drop_without_applying_deltas();
            (output, area)
        };
        let (_, area) = frame(vec![], &app);
        let station = sources["quad"].surface_points()[0];
        // Project with an equivalent unchanged camera; both views use Top defaults.
        let pointer = Viewport::new(ViewKind::Top).project(station, area).unwrap();
        frame(
            vec![
                Event::PointerMoved(pointer),
                Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            &app,
        );
        let (output, _) = frame(
            vec![Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            &app,
        );
        let click = output
            .selection_click
            .expect("source click through offset preview");
        assert_eq!(click.object_id, Some(id));
        app.apply_selection_click(click);
        assert!(
            app.offset_subd_session
                .as_ref()
                .unwrap()
                .edges()
                .iter()
                .all(|e| (e.start().to_array()[2] + 1.).abs() < 1e-10)
        );
        assert!(!app.document.can_undo());
        let Geometry::SubD(source) = app.document.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(source.control_points()[0].to_array(), [0., 0., 0.]);
    }
}
