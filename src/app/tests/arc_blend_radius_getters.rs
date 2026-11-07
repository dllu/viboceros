use super::arc_blend_handles::inputs;
use super::arc_blend_radius::radius;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_radius_getters.rs"]
#[allow(dead_code)]
mod getters;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn radius_targets_match_native_arcs_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in getters::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            if getters::research(row["id"].as_str().unwrap()) {
                continue;
            }
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
            assert_eq!(final_warning, warning, "{id}");
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
                "ARC_BLEND_GETTER_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1,"maximum_native_arc_sample_error":sample_error,"warning":final_warning})
            );
            assert!(error <= 1e-6 && sample_error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 9);
}

#[test]
fn nested_numeric_prompt_suspends_pointer_adjustment_and_escape_returns_one_level() {
    use viboceros_command::arc_blend::Handle;
    let data = native::read("arc_blend_radius_getters");
    let v = &data["results"][0]["value"];
    let mut app = test_app();
    app.document = reference_inputs::setup(v, &v["prescribed_case"]);
    enter(&mut app, "ArcBlend");
    for action in &v["prescribed_case"]["actions"].as_array().unwrap()[..2] {
        app.pick_arc_blend_at(
            reference_inputs::guide(&app.document, action["curve"].as_u64().unwrap() as usize),
            None,
            native::point(&action["pick"]),
        );
    }
    let original = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    app.select_arc_blend_handle(Handle::Ratio);
    let parameter = *app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .adjustment_curve()
        .unwrap()
        .domain()
        .end()
        * 0.25;
    app.hover_arc_blend_parameter(parameter);
    assert_ne!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .preview_input()
            .preview(),
        Some(&original)
    );
    enter(&mut app, "RadiusDifference");
    enter(&mut app, "NaN");
    let s = app.arc_blend_session.as_ref().unwrap();
    assert!(s.radius_prompt);
    assert_eq!(s.input.active_handle(), Some(Handle::Ratio));
    assert!(s.cursor_curve(false).is_none());
    assert_eq!(s.preview_input().preview(), Some(&original));
    assert_eq!(s.input.last_radius_difference(), 0.);
    app.hover_arc_blend_parameter(parameter);
    assert!(!app.accept_arc_blend_parameter(parameter));
    assert_eq!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .preview_input()
            .preview(),
        Some(&original)
    );
    enter(&mut app, "5");
    let s = app.arc_blend_session.as_ref().unwrap();
    assert!(!s.radius_prompt);
    assert_eq!(s.input.active_handle(), Some(Handle::Ratio));
    assert_eq!(s.input.last_radius_difference(), 5.);
    assert_eq!(s.input.preview(), Some(&original));
    enter(&mut app, "");
    assert_eq!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .active_handle(),
        None
    );
    assert_eq!(
        app.arc_blend_session.as_ref().unwrap().input.preview(),
        Some(&original)
    );
    app.select_arc_blend_handle(Handle::Ratio);
    enter(&mut app, "RadiusDifference");
    enter(&mut app, "");
    enter(&mut app, "0");
    let accepted = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    assert_ne!(accepted, original);
    app.select_arc_blend_handle(Handle::Ratio);
    enter(&mut app, "RadiusDifference");
    app.cancel_current_prompt_or_selection();
    let s = app.arc_blend_session.as_ref().unwrap();
    assert!(!s.radius_prompt);
    assert_eq!(s.input.active_handle(), Some(Handle::Ratio));
    assert_eq!(s.input.preview(), Some(&accepted));
    app.cancel_current_prompt_or_selection();
    assert_eq!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .active_handle(),
        None
    );
    assert!(!app.document.can_undo());
    enter(&mut app, "!");
    assert!(app.active_command.is_none());
    assert!(!app.document.can_undo());
}
