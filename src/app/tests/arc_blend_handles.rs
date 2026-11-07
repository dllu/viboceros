use super::command_file::replay;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/arc_blend_handles.rs"]
#[allow(dead_code)]
mod inputs;
use super::arc_blend::{click, frame, scene};
use viboceros_command::arc_blend::Handle;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn source_and_chord_handles_match_native_geometry_attributes_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in inputs::FAMILIES {
        for row in native::read(family)["results"].as_array().unwrap() {
            let v = &row["value"];
            let id = format!("{family}/{}", row["id"].as_str().unwrap());
            if inputs::diagnostic(v).is_some() {
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
                "ARC_BLEND_HANDLE_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            assert!(error <= 1e-6, "{id}: {error:e}");
            count += 1;
        }
    }
    assert_eq!(count, 26);
}
#[test]
fn mouse_handles_preview_accept_cancel_and_commit_without_intermediate_document_edits() {
    let (mut app, ids) = scene();
    let parents = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "ArcBlend");
    app.pick_arc_blend_at(ids[0], None, point(0., 0., 0.));
    app.pick_arc_blend_at(ids[1], None, point(5., 4., 0.));
    let original = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    let screen = app.viewports[0].project(point(0., 0., 0.), rect).unwrap();
    let output = click(&mut app, &context, screen);
    assert_eq!(output.arc_blend_handle, Some(Handle::Start));
    app.handle_viewport_action(output);
    let target = app.viewports[0].project(point(-2., 0., 0.), rect).unwrap();
    let (hover, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(target)]);
    assert!(hover.arc_blend_hover.is_some());
    app.handle_viewport_action(hover);
    let preview = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .preview_input()
        .preview()
        .unwrap()
        .clone();
    assert_ne!(preview, original);
    assert_eq!(
        app.arc_blend_session.as_ref().unwrap().input.preview(),
        Some(&original)
    );
    assert!(!app.document.can_undo());
    let output = click(&mut app, &context, target);
    assert!(output.edge_parameter.is_some());
    app.handle_viewport_action(output);
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
        Some(&preview)
    );
    let p = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .handles()
        .unwrap()[1];
    let screen = app.viewports[0].project(p, rect).unwrap();
    let output = click(&mut app, &context, screen);
    assert_eq!(output.arc_blend_handle, Some(Handle::Ratio));
    app.handle_viewport_action(output);
    enter(&mut app, "2");
    let accepted = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    assert_ne!(accepted, preview);
    app.select_arc_blend_handle(Handle::End);
    enter(&mut app, "CPlane 3Point");
    assert!(app.plane_prompt.is_some());
    assert!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .cursor_curve(app.plane_prompt.is_some())
            .is_none()
    );
    assert!(app.viewport_drafting_input(false, false).active);
    for p in [point(2., 3., 4.), point(3., 3., 4.), point(2., 4., 4.)] {
        app.handle_viewport_action(ViewportOutput {
            picked_point: Some(p),
            ..Default::default()
        });
    }
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .active_handle(),
        Some(Handle::End)
    );
    let p = app.viewports[0].project(point(5., 7., 0.), rect).unwrap();
    let (output, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(p)]);
    app.handle_viewport_action(output);
    app.command_input = "invalid pending adjustment".into();
    app.cancel_current_prompt_or_selection();
    assert!(app.command_input.is_empty());
    assert!(app.active_command.is_some());
    assert_eq!(
        app.arc_blend_session.as_ref().unwrap().input.preview(),
        Some(&accepted)
    );
    for parent in &parents {
        let now = app.document.object(parent.id()).unwrap();
        assert_eq!(now, parent);
        assert!(
            parent
                .geometry_snapshot()
                .shares_storage_with(now.geometry_snapshot())
        );
    }
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document
            .objects()
            .last()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap(),
        accepted
    );
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.can_undo());
}
#[test]
fn press_drag_release_accepts_an_adjustment_and_stale_geometry_clears_the_editor() {
    let (mut app, ids) = scene();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "ArcBlend");
    app.pick_arc_blend_at(ids[0], None, point(0., 0., 0.));
    app.pick_arc_blend_at(ids[1], None, point(5., 4., 0.));
    let p = app.viewports[0].project(point(0., 0., 0.), rect).unwrap();
    let q = app.viewports[0].project(point(-2., 0., 0.), rect).unwrap();
    frame(&mut app, &context, vec![egui::Event::PointerMoved(p)]);
    frame(
        &mut app,
        &context,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    let (output, _) = frame(&mut app, &context, vec![egui::Event::PointerMoved(q)]);
    assert_eq!(output.arc_blend_handle, Some(Handle::Start));
    app.handle_viewport_action(output);
    let (output, _) = frame(
        &mut app,
        &context,
        vec![egui::Event::PointerButton {
            pos: q,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert!(output.edge_parameter.is_some());
    app.handle_viewport_action(output);
    let s = app.arc_blend_session.as_ref().unwrap();
    assert_eq!(s.input.active_handle(), None);
    assert!(
        s.input.handles().unwrap()[0]
            .distance_to(point(-2., 0., 0.))
            .unwrap()
            < 0.05
    );
    assert!(!app.document.can_undo());
    app.select_arc_blend_handle(Handle::End);
    app.document.delete_objects([ids[1]]).unwrap();
    app.validate_arc_blend();
    let s = app.arc_blend_session.as_ref().unwrap();
    assert!(s.input.adjustment_curve().is_none());
    assert!(s.preview_input().preview().is_none());
    assert_eq!(s.input.reference_count(), 0);
    enter(&mut app, "!");
    assert!(app.active_command.is_none());
}
