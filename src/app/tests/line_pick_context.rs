use super::command_file::replay;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/line_pick_context.rs"]
#[allow(dead_code)]
mod inputs;
use super::line_tangent::inputs as tangent_inputs;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn projected_curve_picks_and_tangent_getters_match_native_selection_geometry_history_and_archives()
{
    let sources = inputs::sources();
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            let source = &sources[id];
            if inputs::excluded(id) {
                continue;
            }
            let mut app = test_app();
            app.document = inputs::setup(v, source);
            app.viewports[0].set_construction_plane(native::frame(v));
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            let trace = &v["input_trace"][0];
            if inputs::excluded(id) {
                continue;
            }
            assert_eq!(trace["completed"], trace["expected"]);
            assert_eq!(trace["errors"], serde_json::json!([]));
            enter(&mut app, "Line");
            app.planar = source["planar"] == true;
            for (i, action) in source["actions"].as_array().unwrap().iter().enumerate() {
                assert!(app.active_command.is_some(), "{id}: {:?}", app.command_log);
                if action.get("hover").is_some() {
                    continue;
                }
                if action.get("curve").is_some() {
                    let object =
                        inputs::guide(&app.document, action["curve"].as_u64().unwrap() as usize);
                    app.pick_curve_line_at(object, None, inputs::pick(v, i, action));
                } else if action.get("click").is_some() {
                    let p = inputs::click_point(v, i, &app.curve_line.as_ref().unwrap().input);
                    assert!(app.accept_curve_line_point(p), "{id}");
                } else {
                    let value = inputs::token(action);
                    let value = if action.is_array() {
                        format!("w{value}")
                    } else {
                        value
                    };
                    enter(
                        &mut app,
                        if value == "Enter" {
                            ""
                        } else if value == "Cancel" {
                            "!"
                        } else {
                            &value
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
                "LINE_PICK_CONTEXT_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":archives})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 14);
}
