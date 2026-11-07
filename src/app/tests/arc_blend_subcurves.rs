use super::arc_blend_handles::inputs;
use super::arc_blend_sources::sources;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_subcurves.rs"]
#[allow(dead_code)]
mod subcurves;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn temporary_sources_match_native_pointer_input_geometry_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in subcurves::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let value = &row["value"];
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            let mut app = test_app();
            app.document = reference_inputs::setup(value, &value["prescribed_case"]);
            enter(&mut app, "ArcBlend");
            let mut source = None;
            for (index, action) in value["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if let Some(i) = action["curve"].as_u64() {
                    let guide = reference_inputs::guide(&app.document, i as usize);
                    app.pick_arc_blend_at(guide, None, native::point(&action["pick"]));
                    source = Some(guide);
                } else if let Some(p) = action.get("click") {
                    let s = app.arc_blend_session.as_ref().unwrap();
                    if let Some(curve) = s.cursor_curve(false) {
                        let t = subcurves::pointer_parameter(value, index, curve);
                        app.hover_arc_blend_parameter(t);
                        assert!(app.accept_arc_blend_parameter(t));
                    } else {
                        app.select_arc_blend_handle(inputs::handle(&s.input, native::point(p)));
                    }
                } else if let Some(point) = action.get("hover") {
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
                    app.hover_arc_blend_parameter(rational);
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
                    enter(&mut app, if token == "Enter" { "" } else { &token });
                }
                if app.active_command.is_some() {
                    assert!(!app.document.can_undo(), "{id}");
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            let accepted = value["success"].as_bool().unwrap();
            let (samples, stations) = if accepted {
                sources::samples(&app.document, value)
            } else {
                (0., 0)
            };
            let mut error = inputs::check(&app.document, &value["after"], false);
            if accepted {
                app.document.undo().unwrap();
                error = error.max(inputs::check(&app.document, &value["undo"], false));
                assert!(!app.document.can_undo());
                app.document.redo().unwrap();
                error = error.max(inputs::check(&app.document, &value["redo"], false));
            } else {
                assert!(!app.document.can_undo());
                if value["prescribed_case"]["pending_redo"] == true {
                    app.document.redo().unwrap();
                    error = error.max(inputs::check(&app.document, &value["pending_redo"], false));
                    app.document.undo().unwrap();
                }
            }
            let path = directory.path().join("subcurves.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(inputs::check(&restored, &value["after"], true));
            println!(
                "ARC_BLEND_SUBCURVE_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_arc_sample_error":samples,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 29);
}

#[test]
fn cached_preview_escape_invalid_lengths_and_stale_references_preserve_model_edits() {
    let data = native::read("arc_blend_subcurve_discovery");
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
    enter(&mut app, "ArcBlend");
    enter(&mut app, "SubCrv");
    app.pick_arc_blend_at(id, None, Point3::try_new(-0.25, 0., 0.).unwrap());
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
    assert!(output.arc_blend_hover.is_some());
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
    assert!(app.cancel_arc_blend_adjustment());
    assert!(app.active_command.is_some());
    assert!(app.picking_arc_blend());
    assert!(app.subcurve_draft_preview().is_none());
    enter(&mut app, "SubCrv");
    app.pick_arc_blend_at(id, None, Point3::try_new(-0.25, 0., 0.).unwrap());
    enter(&mut app, "w-4,0,0");
    app.document
        .select_objects_direct([id], viboceros_document::SelectionMode::Replace)
        .unwrap();
    app.commands.execute(&mut app.document, "Delete").unwrap();
    app.validate_arc_blend();
    assert!(app.picking_arc_blend());
    assert!(app.subcurve_draft_preview().is_none());
    assert!(app.document.object(id).is_none());
    enter(&mut app, "!");
    app.document.undo().unwrap();
    assert!(app.document.object(id).is_some());
    assert!(!app.document.can_undo());
}
