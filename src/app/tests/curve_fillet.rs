use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/fillet_getter.rs"]
#[allow(dead_code)]
mod inputs;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn ordered_picks_dynamic_numbers_distances_and_cancellation_match_native() {
    let mut count = 0;
    for family in inputs::FAMILIES {
        let mut previous = viboceros_command::fillet::Preferences::default();
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let options = inputs::initial(value, previous);
            previous = viboceros_command::fillet::Preferences {
                dynamic: false,
                ..inputs::final_options(value, previous)
            };
            if inputs::diagnostic(value) {
                continue;
            }
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            println!("FILLET_CASE {id}");
            let mut app = test_app();
            app.document = inputs::setup(value);
            app.commands.remember_fillet_preferences(options).unwrap();
            let state = inputs::State::capture(&app.document);
            enter(&mut app, "Fillet");
            let geometry_before = app
                .document
                .objects()
                .map(|o| (o.id(), o.geometry().clone()))
                .collect::<Vec<_>>();
            for (index, action) in value["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if action.get("view_snapshot").is_some() {
                    assert!(
                        app.curve_fillet_session
                            .as_ref()
                            .is_some_and(|s| s.scene().is_some()),
                        "{id}: {:?}",
                        app.command_log
                    );
                } else if let Some(p) = action.get("hover") {
                    app.hover_curve_fillet_radius(native::point(p));
                } else if let Some(i) = action["curve"].as_u64() {
                    let click = value["input_trace"][0]["clicks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|c| c["action"].as_u64() == Some(index as u64))
                        .unwrap();
                    app.pick_curve_fillet_at(
                        reference_inputs::guide(&app.document, i as usize),
                        native::point(&click["projected_point"]),
                    );
                } else {
                    let token = reference_inputs::token(action);
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
                if app.curve_fillet_session.is_some() {
                    assert!(!app.document.can_undo(), "{id}: {:?}", app.command_log);
                    assert_eq!(
                        geometry_before,
                        app.document
                            .objects()
                            .map(|o| (o.id(), o.geometry().clone()))
                            .collect::<Vec<_>>(),
                        "{id}"
                    );
                }
            }
            assert!(
                app.curve_fillet_session.is_none(),
                "{id}: {:?}",
                app.command_log
            );
            let mut result = state.verify(&mut app.document, value);
            result["id"] = serde_json::json!(id);
            println!("FILLET_GETTER_UI {result}");
            count += 1;
        }
    }
    assert!(count >= 20);
}

fn ready() -> (VibocerosApp, [ObjectId; 2]) {
    let mut app = test_app();
    for text in ["Line 0,0,0 4,0,0", "Line 4,0,0 4,4,0"] {
        app.commands.execute(&mut app.document, text).unwrap();
    }
    let ids: [ObjectId; 2] = app
        .document
        .objects()
        .map(|o| o.id())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    enter(&mut app, "Fillet Radius=0.5 Dynamic=Yes");
    let context = egui::Context::default();
    let (_, rect) = mouse_frame(&mut app, &context, Vec::new());
    for (index, world) in [
        Point3::try_new(3.2, 0., 0.).unwrap(),
        Point3::try_new(4., 0.8, 0.).unwrap(),
    ]
    .into_iter()
    .enumerate()
    {
        let pos = app.viewports[0].project(world, rect).unwrap();
        mouse_frame(&mut app, &context, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            let (output, _) = mouse_frame(
                &mut app,
                &context,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            if !pressed {
                assert_eq!(
                    output.selection_click.as_ref().unwrap().object_id,
                    Some(ids[index])
                );
            }
            app.handle_viewport_action(output);
        }
    }
    assert!(app.curve_fillet_session.as_ref().unwrap().scene().is_some());
    println!("FILLET_MOUSE_SOURCE_PICKS 2");
    (app, ids)
}

fn mouse_frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let input = ViewportInput {
        drafting: app.viewport_drafting_input(false, false),
        object_filter: app.viewport_object_filter(),
        ..Default::default()
    };
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                rect = ui.available_rect_before_wrap();
                output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
            },
        )
        .drop_without_applying_deltas();
    (output, rect)
}

#[test]
fn distance_hover_reuses_the_candidate_and_acceptance_commits_it_once() {
    let (mut app, ids) = ready();
    assert_eq!(app.document.selected_object_count(), 2);
    assert!(app.viewport_drafting_input(false, false).anchor.is_none());
    enter(&mut app, "w0,0,0");
    let point = Point3::try_new(0., 1.5, 0.).unwrap();
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(point),
        ..Default::default()
    });
    let scene = app.curve_fillet_session.as_ref().unwrap().scene().unwrap();
    let id = scene.objects().last().unwrap().id();
    let cached = scene.object(id).unwrap().geometry_snapshot().clone();
    let curve = scene.object(id).unwrap().geometry().clone();
    app.hover_curve_fillet_radius(point);
    assert!(
        cached.shares_storage_with(
            app.curve_fillet_session
                .as_ref()
                .unwrap()
                .scene()
                .unwrap()
                .object(id)
                .unwrap()
                .geometry_snapshot()
        )
    );
    assert_eq!(app.document.objects().len(), 2);
    assert!(!app.document.can_undo());
    app.handle_viewport_action(ViewportOutput {
        picked_point: Some(point),
        ..Default::default()
    });
    assert!(app.curve_fillet_session.is_some());
    assert_eq!(app.document.objects().len(), 2);
    assert!(
        cached.shares_storage_with(
            app.curve_fillet_session
                .as_ref()
                .unwrap()
                .scene()
                .unwrap()
                .object(id)
                .unwrap()
                .geometry_snapshot()
        )
    );
    enter(&mut app, "");
    assert!(app.curve_fillet_session.is_none());
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.objects().last().unwrap().geometry(), &curve);
    for id in ids {
        assert!(app.document.object(id).is_some());
    }
    assert_eq!(app.document.undo_label(), Some("Fillet"));
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
}

#[test]
fn ready_options_survive_cancellation_and_dynamic_resets_in_the_next_invocation() {
    let rows = native::read("fillet_getter_final");
    let first = rows["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "cancel_changed_preview")
        .unwrap();
    let second = rows["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "remember_preview_radius")
        .unwrap();
    let mut app = test_app();
    app.commands
        .remember_fillet_preferences(inputs::initial(&first["value"], Default::default()))
        .unwrap();
    app.document = inputs::setup(&first["value"]);
    enter(&mut app, "Fillet");
    for (index, action) in first["value"]["prescribed_case"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if let Some(i) = action["curve"].as_u64() {
            let c = first["value"]["input_trace"][0]["clicks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["action"].as_u64() == Some(index as u64))
                .unwrap();
            app.pick_curve_fillet_at(
                reference_inputs::guide(&app.document, i as usize),
                native::point(&c["projected_point"]),
            );
        } else if action.get("view_snapshot").is_none() {
            enter(&mut app, &reference_inputs::token(action));
        }
    }
    assert!(app.curve_fillet_session.is_none());
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    let expected = inputs::initial(&second["value"], Default::default());
    assert_eq!(app.commands.fillet_preferences(), expected);
    app.document = inputs::setup(&second["value"]);
    enter(&mut app, "Fillet");
    assert!(app.picking_curve_fillet());
    assert!(!app.viewport_drafting_input(false, false).active);
    assert!(app.command_log.back().unwrap().contains("Radius=1.5"));
    app.cancel_current_prompt_or_selection();
    assert!(app.curve_fillet_session.is_none());
    assert!(app.document.can_redo());
}

#[test]
fn preview_paints_in_four_views_and_three_display_modes_without_model_edits() {
    let (app, _) = ready();
    let scene = app.curve_fillet_session.as_ref().unwrap().scene().unwrap();
    assert_eq!(scene.selected_object_count(), 2);
    assert_eq!(app.document.objects().len(), 2);
    let before = format!("{:?}", app.document);
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
            for document in [&app.document, scene] {
                let context = egui::Context::default();
                let mut viewport = Viewport::new(kind);
                viewport.display_mode = mode;
                let output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::Vec2::new(800., 600.),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        viewport.show(ui, document, ViewportInput::default(), &[], 0, true);
                    },
                );
                counts.push(
                    viewport
                        .object_scene(
                            egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::Vec2::new(800., 600.),
                            ),
                            document,
                        )
                        .lines
                        .len(),
                );
                output.drop_without_applying_deltas();
            }
            assert!(counts[1] > counts[0], "{kind:?} {mode:?}: {counts:?}");
        }
    }
    assert_eq!(format!("{:?}", app.document), before);
    assert!(!app.document.can_undo());
}
