use super::*;
use crate::viewport::{DisplayMode, ViewKind, ViewportInput, ViewportOutput};
use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};

#[test]
fn standalone_plain_and_ctrl_pointer_picks_preview_loops_in_all_display_modes() {
    let data = capture();
    for mesh in [false, true] {
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
            let (mut app, id) = fixture(&row["value"]);
            enter(&mut app, "SelEdgeLoop");
            let context = Context::default();
            let mut view = Viewport::new(ViewKind::Top);
            view.display_mode = mode;
            let mut time = 0.;
            let mut frame = |events: Vec<Event>, app: &VibocerosApp| {
                time += 0.4;
                let mut area = Rect::NOTHING;
                let mut output = ViewportOutput::default();
                let highlights = app.commands.component_selection().live_picks(&app.document);
                let drawn = context.run_ui(
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
                                object_filter: app.viewport_object_filter(),
                                component_pick: Some(
                                    crate::viewport::ComponentPickFilter::MeshAndSubDEdges,
                                ),
                                component_highlights: &highlights,
                                component_previews: app
                                    .edge_loop_prompt
                                    .as_ref()
                                    .unwrap()
                                    .preview(),
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                );
                let dim = drawn.shapes.iter().any(
                    |s| matches!(&s.shape,egui::Shape::LineSegment{stroke,..}if stroke.width==1.8),
                );
                let bright = drawn.shapes.iter().any(
                    |s| matches!(&s.shape,egui::Shape::LineSegment{stroke,..}if stroke.width==3.),
                );
                drawn.drop_without_applying_deltas();
                (output, area, dim, bright)
            };
            let (_, area, _, _) = frame(Vec::new(), &app);
            let pointer = Viewport::new(ViewKind::Top)
                .project(Point3::try_new(4.5, 3., 0.).unwrap(), area)
                .unwrap();
            for modifiers in [egui::Modifiers::NONE, egui::Modifiers::CTRL] {
                frame(
                    vec![
                        Event::ModifiersChanged(modifiers),
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
                let (output, _, _, _) = frame(
                    vec![Event::PointerButton {
                        pos: pointer,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers,
                    }],
                    &app,
                );
                assert_eq!(
                    output.component_click.as_ref().unwrap().picks,
                    [pick(id, mesh, 11)]
                );
                app.handle_viewport_action(output);
                if modifiers.ctrl {
                    assert!(selected(&app).is_empty());
                    assert!(app.edge_loop_prompt.as_ref().unwrap().preview().is_empty());
                } else {
                    assert_eq!(selected(&app), [11]);
                    assert_eq!(app.edge_loop_prompt.as_ref().unwrap().preview().len(), 4);
                    let (_, _, dim, bright) = frame(Vec::new(), &app);
                    assert!(dim && bright);
                }
            }
            app.cancel_current_prompt_or_selection();
            assert!(selected(&app).is_empty());
            assert!(!app.document.can_undo());
        }
    }
}

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
