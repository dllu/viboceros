use super::*;
use crate::viewport::{DisplayMode, ViewKind, ViewportInput, ViewportOutput};
use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};

#[test]
fn genuine_double_click_events_select_and_deselect_a_loop_in_every_display_mode() {
    let data = capture();
    for mesh in [false, true] {
        for pre in [false, true] {
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let row = data["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"] == if mesh { "mesh_add" } else { "subd_add" })
                    .unwrap();
                let v = &row["value"];
                let (mut app, _id) = fixture(v);
                if !pre {
                    enter(&mut app, "ToNURBS");
                }
                let seed_index = v["seed"].as_u64().unwrap() as usize;
                let edge = indices(&v["edges"][seed_index]);
                let a = &v["input"]["points"][edge[0]];
                let b = &v["input"]["points"][edge[1]];
                let station = Point3::try_from(std::array::from_fn(|i| {
                    (a[i].as_f64().unwrap() + b[i].as_f64().unwrap()) * 0.5
                }))
                .unwrap();
                let context = Context::default();
                let mut view = Viewport::new(ViewKind::Top);
                view.display_mode = mode;
                let mut time = 0.;
                let modifiers = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
                let mut frame = |mut events: Vec<Event>, app: &VibocerosApp| {
                    time += 0.04;
                    events.insert(0, Event::ModifiersChanged(modifiers));
                    let mut area = Rect::NOTHING;
                    let mut output = ViewportOutput::default();
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
                                        component_preselection: app
                                            .component_preselection_available(),
                                        object_filter: app.viewport_object_filter(),
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
                let pointer = Viewport::new(ViewKind::Top).project(station, area).unwrap();
                for repeat in 0..2 {
                    for second in [false, true] {
                        frame(
                            vec![
                                Event::PointerMoved(pointer),
                                Event::PointerButton {
                                    pos: pointer,
                                    button: PointerButton::Primary,
                                    pressed: true,
                                    modifiers,
                                },
                            ],
                            &app,
                        );
                        let (output, _) = frame(
                            vec![Event::PointerButton {
                                pos: pointer,
                                button: PointerButton::Primary,
                                pressed: false,
                                modifiers,
                            }],
                            &app,
                        );
                        assert_eq!(output.component_double_click.is_some(), second);
                        app.handle_viewport_action(output);
                    }
                    if repeat == 0 {
                        assert_eq!(selected(&app), indices(&v["after"]["ordinals"]));
                    } else {
                        assert!(selected(&app).is_empty());
                    }
                    for _ in 0..20 {
                        frame(vec![], &app);
                    }
                }
                assert_eq!(app.document.objects().len(), 1);
                assert!(!app.document.can_undo());
            }
        }
    }
}
