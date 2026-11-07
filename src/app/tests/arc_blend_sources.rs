use super::arc_blend_handles::inputs;
use super::arc_blend_radius::radius;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_sources.rs"]
#[allow(dead_code)]
mod sources;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn nonlinear_contacts_and_numeric_quantities_match_native_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in sources::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if sources::diagnostic(name) {
                continue;
            }
            let value = &row["value"];
            let id = format!("{family}/{name}");
            let mut app = test_app();
            app.document = sources::setup(value);
            enter(&mut app, "ArcBlend");
            let mut target = None;
            for (index, action) in value["prescribed_case"]["actions"]
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
                } else if let Some(p) = action.get("click") {
                    let input = &app.arc_blend_session.as_ref().unwrap().input;
                    if input.active_handle().is_some() {
                        let t = sources::constraint_parameter(value, index, input);
                        assert!(app.accept_arc_blend_parameter(t));
                    } else {
                        app.select_arc_blend_handle(inputs::handle(input, native::point(p)));
                    }
                } else {
                    let token = action["number"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| reference_inputs::token(action));
                    let token = if action.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    if token == "Enter" {
                        let input = &app.arc_blend_session.as_ref().unwrap().input;
                        target = input.radius_difference();
                        sources::check_target(target, sources::target(value));
                    }
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
                if app.active_command.is_some() {
                    assert!(!app.document.can_undo());
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            if target.is_some() {
                sources::check_target(
                    Some(app.commands.arc_blend_preferences().radius_difference),
                    target,
                );
            }
            let warning = app
                .command_log
                .iter()
                .any(|s| s.contains("Unable to achieve"));
            assert_eq!(
                warning,
                value["history"]
                    .as_str()
                    .unwrap()
                    .contains("Unable to achieve")
            );
            let (samples, stations) = sources::samples(&app.document, value);
            let mut error = inputs::check(&app.document, &value["after"], false);
            app.document.undo().unwrap();
            error = error.max(inputs::check(&app.document, &value["undo"], false));
            assert!(!app.document.can_undo());
            app.document.redo().unwrap();
            error = error.max(inputs::check(&app.document, &value["redo"], false));
            let path = directory.path().join("sources.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            assert_eq!(restored.units(), app.document.units());
            error = error.max(inputs::check(&restored, &value["after"], true));
            println!(
                "ARC_BLEND_SOURCE_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"native_stations":stations,"archives":1,"requested_radius_difference":target,"warning":warning})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 35);
}
#[test]
fn rational_pointer_parameters_keep_the_original_arc_contact_and_invalid_quantities_are_atomic() {
    use viboceros_command::arc_blend::Handle;
    let data = native::read("arc_blend_sources");
    let value = &data["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "source_arc_default")
        .unwrap()["value"];
    let mut app = test_app();
    app.document = sources::setup(value);
    enter(&mut app, "ArcBlend");
    for action in &value["prescribed_case"]["actions"].as_array().unwrap()[..2] {
        app.pick_arc_blend_at(
            reference_inputs::guide(&app.document, action["curve"].as_u64().unwrap() as usize),
            None,
            native::point(&action["pick"]),
        );
    }
    let id = reference_inputs::guide(&app.document, 0);
    let curve = app
        .document
        .object(id)
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap();
    let point = Point3::try_new(-3., -1., 0.).unwrap();
    let native = curve
        .closest_parameter(point, app.document.tolerance())
        .unwrap();
    let rational = curve.nurbs_parameter(native).unwrap();
    assert!((native - rational).abs() > 1e-3);
    app.select_arc_blend_handle(Handle::Start);
    let before = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    app.hover_arc_blend_parameter(rational);
    let s = app.arc_blend_session.as_ref().unwrap();
    assert!(
        s.preview_input().handles().unwrap()[0]
            .distance_to(point)
            .unwrap()
            < 1e-10
    );
    assert_eq!(s.input.preview(), Some(&before));
    let context = egui::Context::default();
    super::arc_blend::frame(&mut app, &context, vec![]);
    assert!(app.accept_arc_blend_parameter(rational));
    assert!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .handles()
            .unwrap()[0]
            .distance_to(point)
            .unwrap()
            < 1e-10
    );
    enter(&mut app, "RadiusDifference");
    let saved = app.commands.arc_blend_preferences();
    let preview = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    for token in ["2mm+3mm", "1/0", "NaN", "1e309", "w1,2,3"] {
        enter(&mut app, token);
        let s = app.arc_blend_session.as_ref().unwrap();
        assert!(s.radius_prompt);
        assert_eq!(s.input.preview(), Some(&preview));
        assert_eq!(app.commands.arc_blend_preferences(), saved);
    }
    enter(&mut app, "-0.1cm");
    assert!(!app.arc_blend_session.as_ref().unwrap().radius_prompt);
    assert_eq!(app.commands.arc_blend_preferences().radius_difference, -1.);
    enter(&mut app, "!");
    assert!(!app.document.can_undo());
}

#[test]
fn whole_expression_suffixes_in_points_match_native_line_getters() {
    let directory = tempfile::tempdir().unwrap();
    for row in native::read("arc_blend_point_quantities")["results"]
        .as_array()
        .unwrap()
    {
        let value = &row["value"];
        let id = row["id"].as_str().unwrap();
        let mut app = test_app();
        app.document = sources::setup(value);
        enter(&mut app, "Line");
        for a in value["prescribed_case"]["actions"].as_array().unwrap() {
            enter(&mut app, a["number"].as_str().unwrap());
        }
        assert!(app.active_command.is_none(), "{:?}", app.command_log);
        let mut error = inputs::check(&app.document, &value["after"], false);
        app.document.undo().unwrap();
        error = error.max(inputs::check(&app.document, &value["undo"], false));
        app.document.redo().unwrap();
        error = error.max(inputs::check(&app.document, &value["redo"], false));
        let path = directory.path().join("point-units.3dm");
        app.commands
            .execute(&mut app.document, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut restored = Document::new(app.document.tolerance());
        app.commands
            .execute(&mut restored, &format!("Open3dm {}", path.display()))
            .unwrap();
        error = error.max(inputs::check(&restored, &value["after"], true));
        println!(
            "ARC_BLEND_SOURCE_POINT_UI {}",
            serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
        );
        assert!(error <= 1e-6);
    }
}
