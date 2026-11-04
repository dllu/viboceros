use super::*;
use crate::viewport::{DisplayMode, ViewKind, ViewportInput, ViewportOutput};
use egui::{Context, Event, PointerButton, Pos2, Rect, Vec2};

#[test]
fn ctrl_shift_viewport_picks_convert_edges_in_all_display_modes() {
    for mesh in [false, true] {
        for flat in [false, true] {
            for pre in [false, true] {
                for mode in [
                    DisplayMode::Wireframe,
                    DisplayMode::Shaded,
                    DisplayMode::Ghosted,
                ] {
                    let (mut app, id) = fixture(mesh);
                    app.document.set_subd_control_net(flat);
                    if !pre {
                        enter(&mut app, "ToNURBS");
                    }
                    let Geometry::NurbsCurve(curve) = expected(&app, id, &[0], flat).remove(0)
                    else {
                        panic!()
                    };
                    let domain = curve.domain();
                    let station = curve
                        .evaluate((domain.start() + domain.end()) * 0.5)
                        .unwrap();
                    let context = Context::default();
                    let mut view = Viewport::new(ViewKind::Top);
                    view.display_mode = mode;
                    let mut time = 0.;
                    let modifiers = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
                    let mut frame = |mut events: Vec<Event>, app: &VibocerosApp| {
                        events.insert(0, Event::ModifiersChanged(modifiers));
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
                    let click = output
                        .component_click
                        .expect("Ctrl+Shift routes through component picking");
                    assert!(
                        click.picks.contains(&component(id, mesh, 0)),
                        "{mesh} {flat} {pre} {mode:?}: {click:?}"
                    );
                    let choice = click
                        .picks
                        .iter()
                        .filter(|p| crate::app::nurbs_conversion::eligible(p.kind))
                        .position(|p| *p == component(id, mesh, 0))
                        .unwrap();
                    app.accept_component_click(click);
                    // Resolve a genuine overlap through the existing candidate prompt.
                    if app.component_selection.has_choices() {
                        enter(&mut app, &(choice + 1).to_string());
                    }
                    assert_eq!(
                        app.component_selection
                            .checked_picks(&app.document)
                            .unwrap(),
                        [component(id, mesh, 0)]
                    );
                    if pre {
                        enter(&mut app, "ToNURBS");
                    } else {
                        enter(&mut app, "");
                    }
                    enter(&mut app, "");
                    assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
                    assert_eq!(outputs(&app, 1), [Geometry::NurbsCurve(curve)]);
                }
            }
        }
    }
}
