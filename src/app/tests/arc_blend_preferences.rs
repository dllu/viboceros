use super::arc_blend_handles::inputs;
use super::arc_blend_radius::radius;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_preferences.rs"]
#[allow(dead_code)]
mod preferences;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn ordered_native_settings_are_shared_by_commands_cancellation_and_new_documents() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in preferences::FAMILIES {
        let mut app = test_app();
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if preferences::diagnostic(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            let v = &row["value"];
            app.document = reference_inputs::setup(v, &v["prescribed_case"]);
            enter(&mut app, "ArcBlend");
            assert!(
                app.arc_blend_session
                    .as_ref()
                    .unwrap()
                    .input
                    .radius_difference()
                    .is_none()
            );
            for action in v["prescribed_case"]["actions"].as_array().unwrap() {
                if let Some(i) = action["curve"].as_u64() {
                    app.pick_arc_blend_at(
                        reference_inputs::guide(&app.document, i as usize),
                        None,
                        native::point(&action["pick"]),
                    );
                } else if let Some(p) = action.get("click") {
                    let input = &app.arc_blend_session.as_ref().unwrap().input;
                    app.select_arc_blend_handle(inputs::handle(input, native::point(p)));
                } else {
                    let numeric_cancel =
                        action == "Cancel" && app.arc_blend_session.as_ref().unwrap().radius_prompt;
                    let token = reference_inputs::token(action);
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                    if numeric_cancel {
                        let s = app
                            .arc_blend_session
                            .as_ref()
                            .expect("numeric Cancel retains the parent getter");
                        assert!(!s.radius_prompt);
                        assert_eq!(
                            s.input.radius_difference(),
                            Some(s.input.last_radius_difference())
                        );
                    }
                }
                if app.active_command.is_some() {
                    assert!(!app.document.can_undo());
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            let saved = app.commands.arc_blend_preferences();
            assert_eq!(saved, preferences::expected(name), "{id}");
            let stations = preferences::native_stations(v);
            let samples = if stations > 0 {
                radius::independent_samples(&app.document, v)
            } else {
                0.
            };
            let mut error = inputs::check(&app.document, &v["after"], false);
            let accepted = v["success"].as_bool().unwrap();
            if accepted {
                app.document.undo().unwrap();
                error = error.max(inputs::check(&app.document, &v["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(inputs::check(&app.document, &v["redo"], false));
            } else {
                assert!(!app.document.can_undo());
                if v["prescribed_case"]["pending_redo"] == true {
                    app.document.redo().unwrap();
                    error = error.max(inputs::check(&app.document, &v["pending_redo"], false));
                    app.document.undo().unwrap();
                }
            }
            let path = directory.path().join("settings.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &v["after"], true));
            assert_eq!(app.commands.arc_blend_preferences(), saved);
            println!(
                "ARC_BLEND_PREFERENCES_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 25);
}
#[test]
fn invalid_options_cancelled_numeric_input_and_hover_do_not_replace_accepted_settings() {
    use viboceros_command::arc_blend::Handle;
    let (mut app, ids) = super::arc_blend::scene();
    enter(&mut app, "ArcBlend");
    app.pick_arc_blend_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    app.pick_arc_blend_at(ids[1], None, Point3::try_new(5., 4., 0.).unwrap());
    enter(&mut app, "RadiusDifference");
    enter(&mut app, "2");
    let saved = app.commands.arc_blend_preferences();
    enter(&mut app, "Trim=Yes Garbage=No");
    assert_eq!(app.commands.arc_blend_preferences(), saved);
    app.select_arc_blend_handle(Handle::Ratio);
    enter(&mut app, "RadiusDifference");
    enter(&mut app, "NaN");
    assert!(app.arc_blend_session.as_ref().unwrap().radius_prompt);
    assert_eq!(app.commands.arc_blend_preferences(), saved);
    let context = egui::Context::default();
    super::arc_blend::frame(&mut app, &context, vec![]);
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.arc_blend_preferences(), saved);
    let t = *app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .adjustment_curve()
        .unwrap()
        .domain()
        .end()
        * 0.25;
    app.hover_arc_blend_parameter(t);
    app.cancel_current_prompt_or_selection();
    app.cancel_current_prompt_or_selection();
    assert!(app.active_command.is_none());
    assert_eq!(app.commands.arc_blend_preferences(), saved);
    assert!(!app.document.can_undo());
    enter(&mut app, "ArcBlend");
    let s = app.arc_blend_session.as_ref().unwrap();
    assert_eq!(s.input.preferences(), saved);
    assert_eq!(s.input.radius_difference(), None);
    assert_eq!(
        test_app().commands.arc_blend_preferences(),
        Default::default()
    );
}
