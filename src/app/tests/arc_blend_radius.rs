use super::arc_blend_handles::inputs;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_radius.rs"]
#[allow(dead_code)]
pub(super) mod radius;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn radius_targets_match_native_arcs_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in radius::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let v = &row["value"];
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            if radius::diagnostic(row["id"].as_str().unwrap()).is_some() {
                continue;
            }
            let mut app = test_app();
            app.document = reference_inputs::setup(v, &v["prescribed_case"]);
            enter(&mut app, "ArcBlend");
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
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            let sample_error = radius::independent_samples(&app.document, v);
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
            println!(
                "ARC_BLEND_RADIUS_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1,"maximum_native_arc_sample_error":sample_error})
            );
            assert!(error <= 1e-6 && sample_error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 18);
}

#[test]
fn numeric_prompt_cancellation_invalid_input_and_reference_changes_are_atomic() {
    let data = native::read("arc_blend_radius_targets");
    for row in [&data["results"][0], &data["results"][7]] {
        let v = &row["value"];
        let mut app = test_app();
        app.document = reference_inputs::setup(v, &v["prescribed_case"]);
        enter(&mut app, "ArcBlend");
        for a in &v["prescribed_case"]["actions"].as_array().unwrap()[..2] {
            app.pick_arc_blend_at(
                reference_inputs::guide(&app.document, a["curve"].as_u64().unwrap() as usize),
                None,
                native::point(&a["pick"]),
            );
        }
        let before = app
            .arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .preview()
            .unwrap()
            .clone();
        enter(&mut app, "RadiusDifference");
        enter(&mut app, "NaN");
        assert!(app.arc_blend_session.as_ref().unwrap().radius_prompt);
        app.select_arc_blend_handle(viboceros_command::arc_blend::Handle::Ratio);
        assert!(
            app.arc_blend_session
                .as_ref()
                .unwrap()
                .input
                .active_handle()
                .is_none()
        );
        assert_eq!(
            app.arc_blend_session.as_ref().unwrap().input.preview(),
            Some(&before)
        );
        app.cancel_current_prompt_or_selection();
        assert!(!app.arc_blend_session.as_ref().unwrap().radius_prompt);
        assert!(app.arc_blend_session.as_ref().unwrap().input.ready());
        enter(&mut app, "RadiusDifference");
        enter(&mut app, "30");
        assert!(!app.arc_blend_session.as_ref().unwrap().radius_prompt);
        assert!(
            app.command_log
                .iter()
                .any(|s| s.contains("Unable to achieve"))
        );
        assert!(!app.document.can_undo());
        enter(&mut app, "RadiusDifference");
        let id = reference_inputs::guide(&app.document, 1);
        app.document.delete_objects([id]).unwrap();
        app.validate_arc_blend();
        assert!(!app.arc_blend_session.as_ref().unwrap().radius_prompt);
        assert_eq!(
            app.arc_blend_session
                .as_ref()
                .unwrap()
                .input
                .reference_count(),
            0
        );
        enter(&mut app, "!");
        assert!(app.active_command.is_none());
    }
}
