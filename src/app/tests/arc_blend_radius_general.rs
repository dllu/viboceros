use super::arc_blend_handles::inputs;
use super::arc_blend_radius::radius;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_radius_general.rs"]
mod general;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn radius_targets_match_native_arcs_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in general::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let v = &row["value"];
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            let mut app = test_app();
            app.document = reference_inputs::setup(v, &v["prescribed_case"]);
            enter(&mut app, "ArcBlend");
            let mut final_warning = false;
            for (index, action) in v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if let Some(i) = action["curve"].as_u64() {
                    app.pick_arc_blend_at(
                        reference_inputs::guide(&app.document, i as usize),
                        None,
                        native::point(&action["pick"]),
                    );
                } else if action.get("hover").is_some() {
                    continue;
                } else if let Some(p) = action.get("click") {
                    let input = &app.arc_blend_session.as_ref().unwrap().input;
                    if input.active_handle().is_some() {
                        let t = inputs::parameter(v, index, input);
                        assert!(app.accept_arc_blend_parameter(t));
                    } else {
                        let h = inputs::handle(input, native::point(p));
                        app.select_arc_blend_handle(h);
                    }
                } else {
                    let token = reference_inputs::token(action);
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    if token == "Enter" {
                        final_warning = app
                            .arc_blend_session
                            .as_ref()
                            .unwrap()
                            .input
                            .warning()
                            .is_some();
                    }
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            let warning = app
                .command_log
                .iter()
                .any(|s| s.contains("Unable to achieve"));
            assert_eq!(
                warning,
                v["history"].as_str().unwrap().contains("Unable to achieve"),
                "{id}"
            );
            assert_eq!(final_warning, general::final_warning(v), "{id}");
            let representation_diagnostic =
                general::representation_diagnostic(row["id"].as_str().unwrap());
            let sample_error = if representation_diagnostic {
                general::coincident_circle_error(&app.document, v)
            } else {
                radius::independent_samples(&app.document, v)
            };
            let mut error = inputs::check(&app.document, &v["after"], false);
            assert_eq!(app.document.undo_label(), Some("ArcBlend"));
            app.document.undo().unwrap();
            error = error.max(inputs::check(&app.document, &v["undo"], false));
            assert!(!app.document.can_undo());
            app.document.redo().unwrap();
            error = error.max(inputs::check(&app.document, &v["redo"], false));
            let path = directory.path().join("handles.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &v["after"], true));
            if representation_diagnostic {
                assert!(general::coincident_circle_error(&restored, v) <= 1e-6);
            }
            println!(
                "ARC_BLEND_RADIUS_GENERAL_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1,"maximum_native_arc_sample_error":sample_error,"warning":final_warning,"representation_diagnostic":representation_diagnostic})
            );
            assert!(
                (representation_diagnostic || error <= 1e-6) && sample_error <= 1e-6,
                "{id}: {error:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 79);
}
