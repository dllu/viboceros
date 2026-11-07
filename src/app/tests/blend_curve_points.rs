use super::blend_curve::blend;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/blend_curve_points.rs"]
mod points;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn point_endpoint_ui_matches_native_geometry_sources_and_history() {
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
    for family in points::FAMILIES {
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
            for (index, a) in v["prescribed_case"]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if let Some(i) = a["curve"].as_u64() {
                    let input = &app.blend_curve_session.as_ref().unwrap().input;
                    let point = blend::source_pick(v, index, a, input);
                    app.pick_blend_curve_at(
                        reference_inputs::guide(&app.document, i as usize),
                        None,
                        point,
                    );
                } else if let Some(point) = a.get("click") {
                    let handle = blend::handle(
                        &app.blend_curve_session.as_ref().unwrap().input,
                        native::point(point),
                    );
                    app.select_blend_curve_handle(handle);
                } else {
                    let token = reference_inputs::token(a);
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
                "BLEND_CURVE_POINT_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
            );
            assert!(
                error <= 1e-6 && samples <= 1e-6,
                "{id}: {error:e}, {samples:e}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 41);
}

#[test]
fn windowless_point_preview_acceptance_free_movement_and_cancel_preserve_history() {
    use super::arc_blend::{click, frame, scene};
    use viboceros_command::blend_curve::Handle;
    let (mut app, ids) = scene();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    enter(&mut app, "Point");
    assert!(app.wants_blend_curve_point());
    assert!(
        app.snaps.model_override.is_none(),
        "Point selects the endpoint option before requesting a snap"
    );
    let target = Point3::try_new(4., 3., 0.).unwrap();
    let screen = app.viewports[0].project(target, rect).unwrap();
    let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(screen)]);
    assert!(out.drafting_hover.is_some());
    app.handle_viewport_action(out);
    let s = app.blend_curve_session.as_ref().unwrap();
    assert!(s.input.preview().is_none());
    assert!(s.preview_input().preview().is_some());
    assert!(!app.document.can_undo());
    let out = click(&mut app, &context, screen);
    assert!(out.picked_point.is_some());
    assert!(out.blend_curve_handle.is_none());
    app.handle_viewport_action(out);
    assert!(!app.wants_blend_curve_point());
    let s = app.blend_curve_session.as_ref().unwrap();
    assert_eq!(s.input.continuities(), [2, 0]);
    let before = s.input.preview().unwrap().clone();
    app.select_blend_curve_handle(Handle::StartShape);
    enter(&mut app, "End");
    assert!(
        app.snaps.model_override.is_some(),
        "constrained edits retain one-shot snaps"
    );
    enter(&mut app, "Cancel");
    app.snaps.model_override = None;
    let end = before.evaluate(*before.domain().end()).unwrap();
    assert!(end.distance_to(target).unwrap() < 1e-5);
    let screen = app.viewports[0].project(end, rect).unwrap();
    let out = click(&mut app, &context, screen);
    assert_eq!(out.blend_curve_handle, Some(Handle::End));
    app.handle_viewport_action(out);
    assert!(app.wants_blend_curve_point());
    let moved = Point3::try_new(3., 5., 0.).unwrap();
    let screen = app.viewports[0].project(moved, rect).unwrap();
    let (out, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(screen)]);
    app.handle_viewport_action(out);
    let s = app.blend_curve_session.as_ref().unwrap();
    assert_eq!(s.input.preview(), Some(&before));
    assert_ne!(s.preview_input().preview(), Some(&before));
    enter(&mut app, "Cancel");
    assert_eq!(
        app.blend_curve_session.as_ref().unwrap().input.preview(),
        Some(&before)
    );
    app.select_blend_curve_handle(Handle::End);
    enter(&mut app, "Point");
    assert!(app.snaps.model_override.is_some());
    enter(&mut app, "w4,5,2");
    assert!(app.snaps.model_override.is_none());
    assert_eq!(app.last_point.unwrap().to_array(), [4., 5., 2.]);
    let s = app.blend_curve_session.as_ref().unwrap();
    let curve = s.input.preview().unwrap();
    assert_eq!(
        curve.evaluate(*curve.domain().end()).unwrap().to_array(),
        [4., 5., 2.]
    );
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 4);
    assert_eq!(app.document.undo_label(), Some("BlendCrv"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
    assert!(app.document.object(ids[0]).is_some());
    assert!(!app.document.can_undo());
}

#[test]
fn point_endpoint_object_snap_retains_3d_coordinates_and_the_point_object() {
    use super::arc_blend::{click, frame, scene};
    let (mut app, ids) = scene();
    app.commands
        .execute(&mut app.document, "Point 4,3,2")
        .unwrap();
    app.document.clear_history().unwrap();
    app.osnap = true;
    let context = egui::Context::default();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    enter(&mut app, "Point");
    enter(&mut app, "Point"); // one-shot point snap inside the new point prompt
    assert!(app.snaps.model_override.is_some());
    let target = Point3::try_new(4., 3., 2.).unwrap();
    let screen = app.viewports[0].project(target, rect).unwrap();
    let out = click(&mut app, &context, screen);
    assert_eq!(out.picked_point, Some(target));
    app.handle_viewport_action(out);
    assert!(app.snaps.model_override.is_none());
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(
        app.document
            .objects()
            .filter(|o| matches!(o.geometry(), Geometry::Point(_)))
            .count(),
        2
    );
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 4);
    assert!(!app.document.can_undo());
}

#[test]
fn point_prompt_cancellation_and_stale_sources_keep_ephemeral_state_out_of_history() {
    let (mut app, ids) = super::arc_blend::scene();
    app.commands
        .execute(&mut app.document, "Point 80,90,100")
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    for accept_first in [false, true] {
        enter(&mut app, "BlendCrv");
        enter(&mut app, "Point");
        if accept_first {
            enter(&mut app, "w4,3,2");
        }
        enter(&mut app, "Cancel");
        assert!(app.active_command.is_none());
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!app.document.can_undo());
        assert!(app.document.can_redo());
    }
    enter(&mut app, "BlendCrv");
    enter(&mut app, "Point");
    enter(&mut app, "w4,3,2");
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    assert!(app.blend_curve_session.as_ref().unwrap().input.ready());
    app.document
        .replace_object_geometries([(
            ids[0],
            Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()),
        )])
        .unwrap();
    app.validate_blend_curve();
    let s = app.blend_curve_session.as_ref().unwrap();
    assert_eq!(s.input.reference_count(), 0);
    assert!(s.input.preview().is_none());
    assert!(!s.point_prompt);
    assert!(!app.wants_blend_curve_point());
}
