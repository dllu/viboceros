use super::*;
use crate::orient_edge_test_support as support;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn curve_to_edge_native_getters_preview_and_history() {
    let mut app = test_app();
    let mut failures = Vec::new();
    for (op, v) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (doc, scene) = support::setup(&op, &v);
            app.document = doc;
            app.command_log.clear();
            app.viewports[app.active_viewport].set_construction_plane(support::plane(&op));
            enter(&mut app, "OrientCrvToEdge");
            for action in op["actions"].as_array().unwrap() {
                if let Some(text) = action.as_str() {
                    enter(&mut app, text);
                } else if action.is_array() {
                    enter(
                        &mut app,
                        &format!("w{}", support::token(support::point(action))),
                    );
                } else if action["curve"] == 0 {
                    app.pick_curve_to_edge_at(
                        Some(scene.source),
                        None,
                        Some(support::point(&action["pick"])),
                    );
                } else {
                    app.pick_curve_to_edge_at(Some(scene.target), Some(scene.edge), None);
                }
                if let Some(preview) = app.affine_preview() {
                    let original = format!("{:?}", app.document);
                    let target = point(6., -2., 0.);
                    let map = preview
                        .definition
                        .transform_at(preview.frame.unwrap(), target, app.document.tolerance())
                        .unwrap();
                    assert!(map.transform_point(target).is_ok());
                    assert_eq!(format!("{:?}", app.document), original);
                }
            }
            assert!(app.active_command.is_none(), "{:?}", app.command_log);
            let mut error = support::check(&app.document, &scene, &v, "after_script");
            assert!(app.document.selected_object_ids().next().is_none());
            if app.document.undo_label().is_some() {
                assert_eq!(app.document.undo_label(), Some("OrientCrvToEdge"));
                app.document.undo().unwrap();
                error = error.max(support::check(&app.document, &scene, &v, "undo"));
                assert!(app.document.undo().unwrap().is_none());
                app.document.redo().unwrap();
                error = error.max(support::check(&app.document, &scene, &v, "redo"));
            }
            println!(
                "ORIENT_EDGE_UI {}",
                serde_json::json!({"id":op["id"],"maximum_error":error})
            );
            assert!(error < 1e-6, "{error}");
        }));
        if result.is_err() {
            failures.push(op["id"].clone());
            app.cancel_interactive_command(false);
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn curve_to_edge_cancel_and_changed_captures_preserve_accepted_copies_and_redo() {
    let (op, v) = support::rows().remove(0);
    let (doc, scene) = support::setup(&op, &v);
    let mut app = test_app();
    app.document = doc;
    app.document
        .select_objects_direct([scene.source], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    app.document.clear_selection();
    let before = format!("{:?}", app.document);
    enter(&mut app, "OrientCrvToEdge Copy=Yes");
    app.pick_curve_to_edge_at(
        Some(scene.source),
        None,
        Some(support::point(&op["points"][0])),
    );
    app.pick_curve_to_edge_at(Some(scene.target), Some(scene.edge), None);
    assert!(app.affine_preview().is_some());
    enter(&mut app, "Copy=Maybe");
    assert!(app.orient_edge_session.is_some());
    app.cancel_interactive_command(false);
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "OrientCrvToEdge Copy=Yes");
    app.pick_curve_to_edge_at(Some(scene.source), None, None);
    app.pick_curve_to_edge_at(Some(scene.target), Some(scene.edge), None);
    enter(&mut app, "w3,0,0");
    assert_eq!(app.document.objects().count(), 3);
    app.document
        .set_objects_visibility([scene.target], false)
        .unwrap();
    assert!(!app.validate_orient_edge());
    assert!(app.affine_preview().is_none());
    assert_eq!(app.document.objects().count(), 3);
}

#[test]
fn curve_to_edge_component_picks_and_explicit_shared_face_are_read_only() {
    let (op, v) = support::rows().remove(0);
    let (doc, scene) = support::setup(&op, &v);
    let mut app = test_app();
    app.document = doc;
    enter(&mut app, "OrientCrvToEdge");
    assert!(app.pick_curve_to_edge(Some(scene.source), None));
    app.handle_viewport_action(ViewportOutput {
        edge_click: Some(vec![crate::viewport::EdgePick {
            object: scene.target,
            edge: scene.edge,
        }]),
        ..Default::default()
    });
    assert!(app.affine_preview().is_some());
    assert!(app.document.undo_label().is_none());
    app.document
        .set_objects_visibility([scene.target], false)
        .unwrap();
    let before = format!("{:?}", app.document);
    assert!(!app.validate_orient_edge());
    assert!(app.picking_orient_edge());
    assert!(app.affine_preview().is_none());
    assert_eq!(format!("{:?}", app.document), before);
    app.cancel_interactive_command(false);
    let brep = viboceros_geometry::Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        [[0., 4.], [0., 6.], [0., 2.]],
        app.document.tolerance(),
    )
    .unwrap();
    let face = brep
        .faces()
        .iter()
        .position(|f| {
            f.loops()
                .iter()
                .any(|l| l.trims().iter().any(|t| t.edge() == Some(0)))
        })
        .unwrap();
    let parent = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "OrientCrvToEdge");
    app.pick_curve_to_edge(Some(scene.source), None);
    app.pick_curve_to_edge(Some(parent), Some(0));
    assert!(app.affine_preview().is_none());
    enter(&mut app, &format!("Face={face}"));
    app.pick_curve_to_edge(Some(parent), Some(0));
    assert!(app.affine_preview().is_some(), "{:?}", app.command_log);
    assert!(app.document.undo_label().is_none());
}
