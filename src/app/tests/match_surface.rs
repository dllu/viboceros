use super::boolean_intersection::enter;
use super::merge_surfaces::{captured, workflow};
use super::*;

pub(super) fn frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let picking = app
        .edge_prompt
        .as_ref()
        .is_some_and(edge_commands::EdgePrompt::picking_edge);
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
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        edge_pick: picking,
                        edge_contact_pick: picking,
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    (output, rect)
}
pub(super) fn click(
    app: &mut VibocerosApp,
    context: &egui::Context,
    rect: egui::Rect,
    point: Point3,
) -> ViewportOutput {
    let pos = app.viewports[0].project(point, rect).unwrap();
    frame(app, context, vec![egui::Event::PointerMoved(pos)]);
    let event = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(app, context, vec![event(true)]);
    frame(app, context, vec![event(false)]).0
}
#[test]
fn match_surface_physical_viewport_picks_preview_cancel_and_history_match_native() {
    let mut count = 0;
    for family in ["pilot", "options", "qualified"] {
        let mut app = test_app();
        for row in captured::read(&format!("match_surface_command_{family}"))["results"]
            .as_array()
            .unwrap()
        {
            let id = row["id"].as_str().unwrap();
            if [
                "match_cmd_position",
                "match_cmd_option_menus",
                "match_cmd_tolerance_menu",
                "match_cmd_tolerances_set",
            ]
            .contains(&id)
            {
                continue;
            }
            println!("MATCH_SURFACE_UI_CASE {id}");
            let v = &row["value"];
            app.document = workflow::setup_named(v, "match", "Match");
            workflow::check(&app.document, &v["before"], false);
            let state = workflow::State::capture(&app.document);
            let original = app.document.objects().cloned().collect::<Vec<_>>();
            app.viewports[0] = Viewport::new(ViewKind::Top);
            let context = egui::Context::default();
            frame(&mut app, &context, vec![]);
            let ids = original.iter().skip(1).map(|o| o.id()).collect::<Vec<_>>();
            app.document
                .select_objects_direct(ids.clone(), SelectionMode::Replace)
                .unwrap();
            app.viewports[0]
                .zoom_selected(&app.document, Default::default())
                .unwrap();
            let (_, rect) = frame(&mut app, &context, vec![]);
            enter(&mut app, "_-MatchSrf");
            assert!(!app.command_line_idle());
            for a in v["prescribed_case"]["actions"].as_array().unwrap() {
                if let Some(i) = a["curve"].as_u64() {
                    let out = click(&mut app, &context, rect, captured::point(&a["pick"]));
                    assert_eq!(out.edge_click.as_ref().unwrap().len(), 1, "{id}");
                    assert_eq!(out.edge_contacts.len(), 1, "{id}");
                    assert_eq!(
                        out.edge_contacts[0].0,
                        crate::viewport::EdgePick {
                            object: ids[i as usize],
                            edge: a["edge"].as_u64().unwrap() as usize
                        }
                    );
                    app.handle_viewport_action(out);
                } else if a == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(
                        &mut app,
                        if a == "Enter" {
                            ""
                        } else {
                            a.as_str().unwrap()
                        },
                    );
                }
                if let Some(prompt) = &app.edge_prompt {
                    assert_eq!(
                        app.document.objects().cloned().collect::<Vec<_>>(),
                        original,
                        "{id}"
                    );
                    assert!(!app.document.can_undo());
                    assert!(app.document.can_redo());
                    assert!(prompt.name() == "MatchSrf");
                }
            }
            assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
            let mut record = state.verify_command(&mut app.document, v, "MatchSrf");
            record["id"] = id.into();
            println!("MATCH_SURFACE_UI {record}");
            count += 1;
        }
    }
    assert_eq!(count, 21);
}

#[test]
fn match_surface_ambiguity_menu_defaults_and_stale_preview_are_owned() {
    let data = captured::read("match_surface_command_pilot");
    let mut app = test_app();
    app.document = workflow::setup_named(&data["results"][0]["value"], "match", "Match");
    let ids = app
        .document
        .objects()
        .skip(1)
        .map(|o| o.id())
        .collect::<Vec<_>>();
    let contacts = vec![
        (
            crate::viewport::EdgePick {
                object: ids[0],
                edge: 1,
            },
            Point3::try_new(1., 0.25, 0.0975).unwrap(),
        ),
        (
            crate::viewport::EdgePick {
                object: ids[1],
                edge: 3,
            },
            Point3::try_new(1.12, 0.25, 0.2375).unwrap(),
        ),
    ];
    enter(&mut app, "MatchSrf");
    app.accept_match_contacts(contacts.clone());
    assert!(app.edge_prompt.as_ref().unwrap().scene().is_none());
    enter(&mut app, "2");
    // The second parent's captured contact becomes the source after a numbered choice.
    assert_eq!(
        app.edge_prompt.as_ref().unwrap().highlights()[0].object,
        ids[1]
    );
    app.accept_match_contacts(vec![contacts[0]]);
    assert!(app.edge_prompt.as_ref().unwrap().scene().is_some());
    enter(&mut app, "Mode");
    enter(&mut app, "Position");
    enter(&mut app, "IsocurveDirection");
    enter(&mut app, "Keep");
    enter(&mut app, "Mode");
    enter(&mut app, "");
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    assert_eq!(
        app.commands.match_surface_options().policy.continuity,
        viboceros_geometry::SurfaceBlendContinuity::Position
    );
    app.document.set_tolerance(Tolerance::DEFAULT);
    let state = (app.document.can_undo(), app.document.can_redo());
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    app.validate_match_surface();
    assert!(app.edge_prompt.is_none());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert_eq!((app.document.can_undo(), app.document.can_redo()), state);
    assert_eq!(
        app.commands.match_surface_options().policy.iso,
        viboceros_geometry::SurfaceMatchIso::Preserve
    );
    assert_eq!(
        viboceros_command::CommandRegistry::with_builtins().match_surface_options(),
        Default::default()
    );
}
