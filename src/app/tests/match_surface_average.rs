use super::boolean_intersection::enter;
use super::match_surface::{click, frame};
use super::merge_surfaces::{captured, workflow};
use super::*;

#[test]
fn match_surface_average_viewport_preview_cancellation_and_both_histories_match_native() {
    let mut app = test_app();
    let mut count = 0;
    for row in captured::read("match_surface_average_command")["results"]
        .as_array()
        .unwrap()
    {
        let id = row["id"].as_str().unwrap();
        if id == "match_average_curvature_pending" {
            continue;
        }
        println!("AVERAGE_SURFACE_UI_CASE {id}");
        let v = &row["value"];
        app.document = workflow::setup_named(v, "match", "Match");
        workflow::check(&app.document, &v["before"], false);
        let state = workflow::State::capture(&app.document);
        let original = app.document.objects().cloned().collect::<Vec<_>>();
        let ids = original.iter().skip(1).map(|o| o.id()).collect::<Vec<_>>();
        app.viewports[0] = Viewport::new(ViewKind::Top);
        let context = egui::Context::default();
        frame(&mut app, &context, vec![]);
        app.document
            .select_objects_direct(ids.clone(), SelectionMode::Replace)
            .unwrap();
        app.viewports[0]
            .zoom_selected(&app.document, Default::default())
            .unwrap();
        let (_, rect) = frame(&mut app, &context, vec![]);
        enter(&mut app, "_-MatchSrf");
        for action in v["prescribed_case"]["actions"].as_array().unwrap() {
            if let Some(i) = action["curve"].as_u64() {
                let output = click(&mut app, &context, rect, captured::point(&action["pick"]));
                assert_eq!(output.edge_contacts.len(), 1, "{id}");
                assert_eq!(output.edge_contacts[0].0.object, ids[i as usize]);
                app.handle_viewport_action(output);
            } else if action == "Cancel" {
                app.cancel_current_prompt_or_selection();
            } else {
                enter(
                    &mut app,
                    if action == "Enter" {
                        ""
                    } else {
                        action.as_str().unwrap()
                    },
                );
            }
            if app.edge_prompt.is_some() {
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    original,
                    "{id}"
                );
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
                if app.commands.match_surface_options().average
                    && let Some(scene) = app
                        .edge_prompt
                        .as_ref()
                        .and_then(edge_commands::EdgePrompt::scene)
                {
                    for (i, id) in ids.iter().enumerate() {
                        assert_ne!(
                            scene.object(*id).unwrap().geometry(),
                            original[i + 1].geometry(),
                            "{id}"
                        );
                    }
                }
            }
        }
        assert!(app.command_line_idle(), "{id}: {:?}", app.command_log);
        let mut record = state.verify_command(&mut app.document, v, "MatchSrf");
        record["id"] = id.into();
        println!("AVERAGE_SURFACE_UI {record}");
        count += 1;
    }
    assert_eq!(count, 7);
}
