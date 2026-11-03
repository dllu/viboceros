use super::*;
#[test]
fn extrude_subd_viewport_pointer_pick_and_preview_work_in_each_display_mode() {
    use crate::viewport::{
        ComponentPickFilter, DisplayMode, ViewKind, Viewport, ViewportInput, ViewportOutput,
    };
    use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};
    let sources = support::sources();
    for mode in [
        DisplayMode::Wireframe,
        DisplayMode::Shaded,
        DisplayMode::Ghosted,
    ] {
        for edge in [false, true] {
            let mut app = test_app();
            let (doc, id) = support::fixture(&sources["quad"]);
            app.document = doc;
            enter(&mut app, "ExtrudeSubD");
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
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(800., 600.),
                            )),
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
                                    component_pick: app
                                        .picking_extrude_subd()
                                        .then_some(ComponentPickFilter::SubDExtrude),
                                    extrude_subd_edges: app
                                        .extrude_subd_session
                                        .as_ref()
                                        .map(|s| s.edges()),
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
            let station = if edge {
                let lines = sources["quad"].display_edge_polylines().unwrap();
                let l = &lines[0];
                l[l.len() / 2]
            } else {
                Point3::try_new(2., 1.5, 0.).unwrap()
            };
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
            assert!(output.selection_click.is_none() && output.picked_point.is_none());
            let click = output.component_click.unwrap();
            assert_eq!(
                click.picks,
                [ComponentPick {
                    object: id,
                    index: 0,
                    kind: if edge {
                        ComponentSelectionKind::SubDEdge
                    } else {
                        ComponentSelectionKind::SubDFace
                    }
                }]
            );
            app.accept_component_click(click);
            enter(&mut app, "");
            assert!(!app.picking_extrude_subd());
            assert!(
                !app.extrude_subd_session
                    .as_ref()
                    .unwrap()
                    .edges()
                    .is_empty()
            );
            frame(vec![], &app);
            assert!(!app.document.can_undo());
            enter(&mut app, "");
            assert_eq!(app.document.undo_label(), Some("ExtrudeSubD"));
        }
    }
}
