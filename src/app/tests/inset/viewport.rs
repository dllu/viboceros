use super::*;
#[test]
fn inset_pointer_face_picks_and_previews_work_in_all_display_modes() {
    use crate::viewport::{
        ComponentPickFilter, DisplayMode, ViewKind, Viewport, ViewportInput, ViewportOutput,
    };
    use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};
    for family in [
        "inset_faces",
        "inset_mesh_ready",
        "inset_surface_sdk",
        "inset_planar_curves",
    ] {
        let native = common::cases_for(family).remove(0).1;
        for mode in [
            DisplayMode::Wireframe,
            DisplayMode::Shaded,
            DisplayMode::Ghosted,
        ] {
            let mut app = test_app();
            let (doc, id) = if matches!(family, "inset_surface_sdk" | "inset_planar_curves") {
                native_surface::fixture(&native["before"])
            } else {
                support::fixture(&native)
            };
            app.document = doc;
            enter(&mut app, "Inset");
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
                                        .picking_inset()
                                        .then_some(ComponentPickFilter::InsetFaces),
                                    inset_edges: app.inset_session.as_ref().map(|s| s.edges()),
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
            let pointer = Viewport::new(ViewKind::Top)
                .project(Point3::try_new(2., 1.5, 0.).unwrap(), area)
                .unwrap();
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
            assert_eq!(click.picks.len(), 1);
            assert_eq!(click.picks[0].object, id);
            app.accept_component_click(click);
            enter(&mut app, "");
            assert!(!app.picking_inset());
            enter(&mut app, ".3");
            frame(vec![], &app);
            assert!(!app.document.can_undo());
            enter(&mut app, "");
            assert_eq!(app.document.undo_label(), Some("Inset"));
        }
    }
}
