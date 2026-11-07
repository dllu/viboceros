use super::arc_blend_handles::inputs;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/blend_curve.rs"]
#[allow(dead_code)]
mod blend;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn adjustable_previews_contacts_handles_and_history_match_native() {
    replay_workflows(false);
}
#[test]
fn higher_order_blends_match_native_sources_shape_edits_and_history() {
    replay_workflows(true);
}
fn replay_workflows(high: bool) {
    let directory = tempfile::tempdir().unwrap();
    let domain_error = std::cell::Cell::new(0_f64);
    let check = |doc: &Document, target: &serde_json::Value, archive| {
        if high {
            let (error, domain) = blend::check_high(doc, target, archive);
            domain_error.set(domain_error.get().max(domain));
            assert!(
                domain <= 2e-6,
                "native approximate length domain: {domain:e}"
            );
            error
        } else {
            inputs::check(doc, target, archive)
        }
    };
    let mut count = 0;
    for family in if high {
        blend::HIGH_FAMILIES
    } else {
        blend::FAMILIES
    } {
        for row in native::read(family)["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            if if high {
                !blend::high(name)
            } else {
                blend::diagnostic(name)
            } {
                continue;
            }
            let id = format!("{family}/{name}");
            domain_error.set(0.);
            let v = &row["value"];
            let mut app = test_app();
            app.document = reference_inputs::setup(v, &v["prescribed_case"]);
            let before_jets = if high {
                blend::derivatives(&app.document, &v["before"])
            } else {
                (0., 0., 0)
            };
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
            let after_jets = if high {
                blend::derivatives(&app.document, &v["after"])
            } else {
                (0., 0., 0)
            };
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
                "{} {}",
                if high {
                    "BLEND_CURVE_UI_HIGH"
                } else {
                    "BLEND_CURVE_UI"
                },
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"maximum_native_sample_error":samples,"maximum_parameter_domain_error":domain_error.get(),"maximum_normalized_derivative_error":derivative_error,"maximum_absolute_derivative_error":before_jets.0.max(after_jets.0),"derivative_stations":before_jets.2+after_jets.2,"native_stations":stations,"archives":1,"accepted":accepted})
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
fn windowless_source_clicks_shape_hover_and_acceptance_create_one_history_record() {
    for order in 2..=4 {
        windowless_handles(order);
    }
}
fn windowless_handles(order: u8) {
    use super::arc_blend::{click, frame, scene};
    use viboceros_command::blend_curve::Handle;
    let (mut app, ids) = scene();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "BlendCrv");
    enter(&mut app, &format!("Continuity_1=G{order}"));
    enter(&mut app, &format!("Continuity_2=G{order}"));
    for world in [
        Point3::try_new(-0.25, 0., 0.).unwrap(),
        Point3::try_new(5., 4.25, 0.).unwrap(),
    ] {
        let screen = app.viewports[0].project(world, rect).unwrap();
        let output = click(&mut app, &context, screen);
        app.handle_viewport_action(output);
    }
    let input = &app.blend_curve_session.as_ref().unwrap().input;
    assert!(input.ready());
    let shape = input
        .control_points()
        .into_iter()
        .find(|(h, _)| *h == Handle::StartShape)
        .unwrap()
        .1;
    let screen = app.viewports[0].project(shape, rect).unwrap();
    let output = click(&mut app, &context, screen);
    assert_eq!(output.blend_curve_handle, Some(Handle::StartShape));
    app.handle_viewport_action(output);
    let before = app
        .blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    let target = Point3::try_new(2.5, 0., 0.).unwrap();
    let screen = app.viewports[0].project(target, rect).unwrap();
    let (output, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(screen)]);
    assert!(output.blend_curve_hover.is_some());
    app.handle_viewport_action(output);
    let s = app.blend_curve_session.as_ref().unwrap();
    assert_eq!(s.input.preview(), Some(&before));
    assert_ne!(s.preview_input().preview(), Some(&before));
    assert!(!app.document.can_undo());
    let output = click(&mut app, &context, screen);
    assert!(output.blend_curve_parameter.is_some());
    app.handle_viewport_action(output);
    let p = app
        .blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .control_points()
        .into_iter()
        .find(|(h, _)| *h == Handle::StartShape)
        .unwrap()
        .1;
    assert!(p.distance_to(target).unwrap() < 1e-5);
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
fn cancelled_adjustments_invalid_lengths_and_stale_sources_preserve_model_history() {
    use viboceros_command::blend_curve::Handle;
    let (mut app, ids) = super::arc_blend::scene();
    enter(&mut app, "BlendCrv");
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(0., 0., 0.).unwrap());
    app.pick_blend_curve_at(ids[1], None, Point3::try_new(5., 4., 0.).unwrap());
    let accepted = app
        .blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    app.select_blend_curve_handle(Handle::StartShape);
    for token in ["NaN", "1/0", "-1", "0"] {
        enter(&mut app, token);
        assert_eq!(
            app.blend_curve_session.as_ref().unwrap().input.preview(),
            Some(&accepted)
        );
        assert!(!app.document.can_undo());
    }
    app.hover_blend_curve_parameter(4.);
    assert!(app.cancel_blend_curve_adjustment());
    assert_eq!(
        app.blend_curve_session.as_ref().unwrap().input.preview(),
        Some(&accepted)
    );
    enter(
        &mut app,
        "Continuity_1=G1 Trim=Yes Join=Yes BlendStart=PickPoint",
    );
    enter(&mut app, "!");
    enter(&mut app, "BlendCrv");
    let prefs = app
        .blend_curve_session
        .as_ref()
        .unwrap()
        .input
        .preferences();
    assert!(prefs.trim && prefs.join && prefs.pick_point);
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .continuities(),
        [2; 2]
    );
    app.pick_blend_curve_at(ids[0], None, Point3::try_new(-1., 0., 0.).unwrap());
    app.document
        .select_objects_direct([ids[0]], SelectionMode::Replace)
        .unwrap();
    app.commands.execute(&mut app.document, "Delete").unwrap();
    app.validate_blend_curve();
    assert_eq!(
        app.blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .reference_count(),
        0
    );
    assert!(app.document.object(ids[0]).is_none());
    enter(&mut app, "!");
    app.document.undo().unwrap();
    assert!(app.document.object(ids[0]).is_some());
    assert!(!app.document.can_undo());
}
