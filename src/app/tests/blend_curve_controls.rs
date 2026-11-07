use super::arc_blend_subcurves::subcurves::pointer_parameter;
use super::blend_curve::blend;
use super::blend_curve_edges::edges;
use super::blend_curve_points::points;
use super::blend_curve_symmetry::measurements;
use super::line_reference::inputs as reference_inputs;
#[path = "../../../crates/viboceros-command/tests/support/blend_curve_controls.rs"]
#[allow(dead_code)]
mod families;
use super::plane_surface_native_support as native;
use super::*;
mod interaction;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn higher_controls_ui_match_native_geometry_sources_and_history() {
    replay_workflows();
}
fn replay_workflows() {
    let directory = tempfile::tempdir().unwrap();
    let diagnostic = std::cell::Cell::new(false);
    let domain_error = std::cell::Cell::new(0_f64);
    let domain_ratio = std::cell::Cell::new(0_f64);
    let check = |doc: &Document, target: &serde_json::Value, archive| {
        let (error, domain) = blend::check_high(doc, target, archive);
        domain_error.set(domain_error.get().max(domain));
        let ratio = measurements::parameter_domain_ratio(doc, target);
        domain_ratio.set(domain_ratio.get().max(ratio));
        assert!(
            diagnostic.get() || ratio <= 1.,
            "Native fractional domain precision: {domain:e}, ratio={ratio:e}"
        );
        error.max(edges::parent_edges(doc, target))
    };
    let mut count = 0;
    for family in families::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if !families::included(family, name) {
                continue;
            }
            let id = format!("{family}/{name}");
            diagnostic.set(families::domain_diagnostic(family, name));
            domain_error.set(0.);
            domain_ratio.set(0.);
            let v = &row["value"];
            let mut app = test_app();
            app.document = edges::setup(v);
            let identities = points::identities(&app.document);
            let before_jets = blend::derivatives(&app.document, &v["before"]);
            app.commands
                .remember_blend_curve_preferences(edges::preferences(v));
            enter(&mut app, "BlendCrv");
            let mut source = None;
            for (index, a) in v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if a.get("wait_prompt").is_some() {
                    continue;
                } else if let Some(i) = a["curve"].as_u64() {
                    let input = &app.blend_curve_session.as_ref().unwrap().input;
                    let point = blend::source_pick(v, index, a, input);
                    source = Some(reference_inputs::guide(&app.document, i as usize));
                    app.pick_blend_curve_at(
                        reference_inputs::guide(&app.document, i as usize),
                        blend::source_edge(v, index, a),
                        point,
                    );
                } else if a.get("click").is_some() {
                    let session = app.blend_curve_session.as_ref().unwrap();
                    if session.input.angle_constraint().is_some() {
                        let point = blend::angle_point(&session.input, v, index);
                        app.hover_blend_curve_angle_point(point);
                        assert!(app.accept_blend_curve_angle_point(point));
                    } else if session.input.shape_constraint().is_some() {
                        let t = blend::shape_parameter(&session.input, v, index);
                        app.hover_blend_curve_parameter(t);
                        assert!(app.accept_blend_curve_parameter(t));
                    } else if let Some(curve) = session.cursor_curve() {
                        let t = pointer_parameter(v, index, curve);
                        app.hover_blend_curve_parameter(t);
                        assert!(app.accept_blend_curve_parameter(t));
                    } else {
                        let handle = blend::handle_at_click(&session.input, v, index);
                        app.select_blend_curve_handle_with_modifiers(
                            handle,
                            a["shift"].as_bool().unwrap_or(false),
                            a["alt"].as_bool().unwrap_or(false),
                        );
                    }
                } else if let Some(point) = a.get("hover") {
                    let curve = app
                        .document
                        .object(source.unwrap())
                        .unwrap()
                        .geometry()
                        .curve_ref()
                        .unwrap();
                    let t = curve
                        .closest_parameter(native::point(point), app.document.tolerance())
                        .unwrap();
                    let rational = curve.nurbs_parameter(t).unwrap();
                    app.hover_blend_curve_parameter(rational);
                } else {
                    let token = a["number"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| reference_inputs::token(a));
                    let token = if a.is_array() {
                        format!("w{token}")
                    } else {
                        token
                    };
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
                if app.active_command.is_some() {
                    assert!(!app.document.can_undo(), "{id}");
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            let accepted = blend::accepted(v);
            let (samples, stations) = if accepted {
                blend::samples(&app.document, v)
            } else {
                (0., 0)
            };
            points::check_identities(&app.document, &identities, &v["after"]);
            let after_jets = blend::derivatives(&app.document, &v["after"]);
            let derivative_error = before_jets.1.max(after_jets.1);
            assert!(
                derivative_error <= 1e-6,
                "{id}: normalized derivative error {derivative_error:e}"
            );
            let mut error = check(&app.document, &v["after"], false);
            if accepted {
                app.document.undo().unwrap();
                error = error.max(check(&app.document, &v["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(check(&app.document, &v["redo"], false));
            } else {
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
                app.document.redo().unwrap();
                error = error.max(check(&app.document, &v["pending_redo"], false));
                app.document.undo().unwrap();
            }
            let path = directory.path().join("blend.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(check(&restored, &v["after"], true));
            println!(
                "BLEND_CURVE_CONTROLS_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_parameter_domain_gate_ratio":domain_ratio.get(),"nominal_capture":!diagnostic.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 77);
}
