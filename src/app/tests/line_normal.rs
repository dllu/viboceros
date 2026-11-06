use super::command_file::replay;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/line_normal.rs"]
#[allow(dead_code)]
mod inputs;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn line_normal_getters_match_native_selection_cancellation_geometry_history_and_archives() {
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            let source = &sources[id];
            let mut app = test_app();
            app.document = inputs::setup(v, source);
            app.viewports[0].set_construction_plane(native::frame(v));
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            let trace = &v["input_trace"][0];
            if inputs::diagnostic(id) {
                continue;
            }
            assert_eq!(trace["completed"], trace["expected"]);
            assert_eq!(trace["errors"], serde_json::json!([]));
            enter(&mut app, "Line");
            for action in source["actions"].as_array().unwrap() {
                assert!(app.line_input.is_some(), "{id} {:?}", app.command_log);
                if action.get("hover").is_some() {
                    app.handle_viewport_action(ViewportOutput {
                        drafting_hover: Some(native::point(&action["hover"])),
                        ..Default::default()
                    });
                } else if action.is_object() {
                    assert!(app.picking_line_normal());
                    let object =
                        inputs::guide(&app.document, action["curve"].as_u64().unwrap() as usize);
                    assert!(app.pick_line_normal_at(
                        object,
                        viboceros_command::line::normal::Component::Object,
                        native::point(&action["pick"])
                    ));
                } else {
                    let token = inputs::token(action);
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    enter(
                        &mut app,
                        if token == "Enter" {
                            ""
                        } else if token == "Cancel" {
                            "!"
                        } else {
                            &token
                        },
                    );
                }
                if app.active_command.is_some() {
                    assert_eq!(app.document.objects().len(), parents.len());
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let mut error = replay::check(&app.document, &v["after"], false);
            let mut archives = 0;
            assert_eq!(app.document.can_undo(), v["success"] == true);
            if app.document.can_undo() {
                assert_eq!(app.document.undo_label(), Some("Line"));
                app.document.undo().unwrap();
                error = error.max(replay::check(&app.document, &v["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, &v["redo"], false));
                let path = directory.path().join("extension.3dm");
                app.commands
                    .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                    .unwrap();
                let mut restored = Document::new(app.document.tolerance());
                app.commands
                    .execute(&mut restored, &format!("Open3dm {}", path.display()))
                    .unwrap();
                error = error.max(replay::check(&restored, &v["after"], true));
                archives = 1;
            }
            if v["pending_redo"].is_array() {
                assert!(app.document.can_redo());
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, &v["pending_redo"], false));
            }
            for parent in parents {
                let now = app.document.object(parent.id()).unwrap();
                assert_eq!(now, &parent);
                assert!(
                    parent
                        .geometry_snapshot()
                        .shares_storage_with(now.geometry_snapshot())
                );
            }
            println!(
                "LINE_NORMAL_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":archives})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 46);
}
