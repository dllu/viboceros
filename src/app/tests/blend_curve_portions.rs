use super::arc_blend_subcurves::subcurves::pointer_parameter;
use super::blend_curve::blend;
use super::blend_curve_points::points;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/blend_curve_portion_families.rs"]
mod families;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn temporary_curve_portion_ui_matches_native_geometry_sources_and_history() {
    replay_workflows();
}
fn replay_workflows() {
    let directory = tempfile::tempdir().unwrap();
    let domain_error = std::cell::Cell::new(0_f64);
    let check = |doc: &Document, target: &serde_json::Value, archive| {
        let (error, domain) = blend::check_high(doc, target, archive);
        domain_error.set(domain_error.get().max(domain));
        assert!(
            domain <= 2e-6,
            "native approximate length domain: {domain:e}"
        );
        error
    };
    let mut count = 0;
    for family in families::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if points::diagnostic(name) {
                continue;
            }
            let id = format!("{family}/{name}");
            domain_error.set(0.);
            let v = &row["value"];
            let mut app = test_app();
            app.document = reference_inputs::setup(v, &v["prescribed_case"]);
            let identities = points::identities(&app.document);
            let before_jets = blend::derivatives(&app.document, &v["before"]);
            app.commands
                .remember_blend_curve_preferences(points::preferences(v));
            enter(&mut app, "BlendCrv");
            let mut source = None;
            for (index, a) in v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if let Some(i) = a["curve"].as_u64() {
                    let input = &app.blend_curve_session.as_ref().unwrap().input;
                    let point = blend::source_pick(v, index, a, input);
                    source = Some(reference_inputs::guide(&app.document, i as usize));
                    app.pick_blend_curve_at(
                        reference_inputs::guide(&app.document, i as usize),
                        None,
                        point,
                    );
                } else if let Some(point) = a.get("click") {
                    let session = app.blend_curve_session.as_ref().unwrap();
                    if let Some(curve) = session.cursor_curve() {
                        let t = pointer_parameter(v, index, curve);
                        app.hover_blend_curve_parameter(t);
                        assert!(app.accept_blend_curve_parameter(t));
                    } else {
                        let handle = blend::handle(&session.input, native::point(point));
                        app.select_blend_curve_handle(handle);
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
                "BLEND_CURVE_PORTION_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 36);
}

#[test]
fn cached_preview_escape_invalid_lengths_and_stale_references_preserve_model_edits() {
    let data = native::read("blend_curve_portion_pilot");
    let value = &data["results"][0]["value"];
    let mut app = test_app();
    app.document = reference_inputs::setup(value, &value["prescribed_case"]);
    let id = reference_inputs::guide(&app.document, 0);
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    super::arc_blend::frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = super::arc_blend::frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    enter(&mut app, "SubCrv");
    app.pick_blend_curve_at(id, None, Point3::try_new(-0.25, 0., 0.).unwrap());
    let start = app.viewports[0]
        .project(Point3::try_new(-4., 0., 0.).unwrap(), rect)
        .unwrap();
    let output = super::arc_blend::click(&mut app, &context, start);
    assert!(output.edge_parameter.is_some());
    app.handle_viewport_action(output);
    let end = app.viewports[0]
        .project(Point3::try_new(-1., 0., 0.).unwrap(), rect)
        .unwrap();
    let (output, _) =
        super::arc_blend::frame(&mut app, &context, vec![egui::Event::PointerMoved(end)]);
    assert!(output.blend_curve_hover.is_some());
    app.handle_viewport_action(output);
    let first = app.subcurve_draft_preview().unwrap().geometry;
    super::arc_blend::frame(&mut app, &context, vec![]);
    let second = app.subcurve_draft_preview().unwrap().geometry;
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    for token in ["20mm", "NaN", "1/0", "1e309"] {
        enter(&mut app, token);
        assert!(std::sync::Arc::ptr_eq(
            &first,
            &app.subcurve_draft_preview().unwrap().geometry
        ));
        assert!(!app.document.can_undo());
    }
    assert!(app.cancel_blend_curve_adjustment());
    assert!(app.active_command.is_some());
    assert!(app.picking_blend_curve());
    assert!(app.subcurve_draft_preview().is_none());
    enter(&mut app, "SubCrv");
    app.pick_blend_curve_at(id, None, Point3::try_new(-0.25, 0., 0.).unwrap());
    enter(&mut app, "w-4,0,0");
    app.document
        .select_objects_direct([id], viboceros_document::SelectionMode::Replace)
        .unwrap();
    app.commands.execute(&mut app.document, "Delete").unwrap();
    app.validate_blend_curve();
    assert!(app.picking_blend_curve());
    assert!(app.subcurve_draft_preview().is_none());
    assert!(app.document.object(id).is_none());
    enter(&mut app, "!");
    app.document.undo().unwrap();
    assert!(app.document.object(id).is_some());
    assert!(!app.document.can_undo());
}

#[test]
fn windowless_temporary_endpoint_snap_is_consumed_without_editing_the_parent() {
    use super::arc_blend::{click, frame};
    let value = &native::read("blend_curve_portion_pilot")["results"][0]["value"];
    let mut app = test_app();
    app.document = reference_inputs::setup(value, &value["prescribed_case"]);
    let id = reference_inputs::guide(&app.document, 0);
    let parent = app.document.object(id).unwrap().clone();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    enter(&mut app, "SubCrv");
    enter(&mut app, &format!("Curve={id}"));
    enter(&mut app, "w-4,0,0");
    enter(&mut app, "End");
    assert!(app.snaps.model_override.is_some());
    let screen = app.viewports[0]
        .project(Point3::try_new(0., 0., 0.).unwrap(), rect)
        .unwrap();
    let output = click(&mut app, &context, screen);
    assert!(output.blend_curve_parameter.is_some());
    app.handle_viewport_action(output);
    assert!(app.snaps.model_override.is_none());
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .reference_count(),
        1
    );
    let now = app.document.object(id).unwrap();
    assert_eq!(now, &parent);
    assert!(
        parent
            .geometry_snapshot()
            .shares_storage_with(now.geometry_snapshot())
    );
    assert!(!app.document.can_undo());
    enter(&mut app, "!");
}
