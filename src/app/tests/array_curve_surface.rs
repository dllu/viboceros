use super::*;
use crate::array_curve_surface_test_support as support;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn surface_curve_array_ui_replays_native_inputs_selection_groups_and_one_undo() {
    let mut app = test_app();
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (doc, scene) = support::setup(&value);
            app.document = doc;
            let id = op["id"].as_str().unwrap();
            app.viewports[app.active_viewport].set_construction_plane(support::plane(&op));
            enter(&mut app, "ArrayCrvOnSrf");
            for (i, action) in value["actions"].as_array().unwrap().iter().enumerate() {
                if let Some(text) = action.as_str() {
                    enter(&mut app, text);
                } else if action.is_array() {
                    let p = support::point(action);
                    if let Some(preview) = app.affine_preview() {
                        let before = format!("{:?}", app.document);
                        let transform = preview
                            .definition
                            .transform_at(preview.frame.unwrap(), p, app.document.tolerance())
                            .unwrap();
                        app.update_affine_preview(Some(transform));
                        assert_eq!(
                            format!("{:?}", app.document),
                            before,
                            "{id} preview mutated document"
                        );
                    }
                    enter(&mut app, &format!("w{}", support::token(p)));
                } else if let Some(n) = action.as_f64() {
                    enter(&mut app, &n.to_string());
                } else {
                    let index = action["curve"].as_u64().unwrap() as usize;
                    if index >= 2 {
                        enter(&mut app, &format!("SelID {}", scene.sources[index - 2]));
                    } else if index == 1 {
                        assert!(
                            app.accept_array_surface(scene.target, None),
                            "{id} {i} {:?}",
                            app.command_log
                        );
                    } else {
                        let pick = value["input_trace"]["clicks"]
                            .as_array()
                            .and_then(|c| c.iter().find(|c| c["action"] == i))
                            .map(|c| support::point(&c["projected_point"]));
                        assert!(
                            app.pick_array_curve_at(Some(scene.rail), None, pick),
                            "{id} {i} {:?}",
                            app.command_log
                        );
                    }
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let mut max = scene.check(&app.document, &value, "after_script", id);
            if app.document.objects().count() > value["before"]["objects"].as_array().unwrap().len()
            {
                assert_eq!(app.document.undo_label(), Some("ArrayCrvOnSrf"));
                app.document.undo().unwrap();
                max = max.max(scene.check(&app.document, &value, "undo", id));
                assert!(app.document.undo().unwrap().is_none());
                app.document.redo().unwrap();
                max = max.max(scene.check(&app.document, &value, "redo", id));
            } else {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "ARRAY_CURVE_SURFACE_UI_AUDIT {}",
                serde_json::json!({"id":id,"maximum_error":max})
            );
        }));
        if result.is_err() {
            failures.push(op["id"].clone());
            app.cancel_interactive_command(false);
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn array_curve_surface_stale_guides_discard_previews_without_mutating_accepted_copies() {
    let (op, value) = support::rows().into_iter().next().unwrap();
    let (doc, scene) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    enter(&mut app, "ArrayCrvOnSrf");
    enter(&mut app, "w0,0,0");
    app.pick_array_curve_at(Some(scene.rail), None, None);
    app.accept_array_surface(scene.target, None);
    enter(&mut app, "w0,2,5");
    let accepted = app.document.objects().count();
    app.document
        .set_objects_visibility([scene.rail], false)
        .unwrap();
    assert!(!app.validate_array_curve_surface());
    assert!(app.picking_array_curve());
    assert!(app.affine_preview().is_none());
    assert_eq!(app.document.objects().count(), accepted);
    enter(&mut app, "Cancel");
    assert_eq!(op["id"], "array_srf_0");
}

#[test]
fn array_curve_surface_edges_faces_cancellation_and_numeric_defaults_preserve_history() {
    let (_, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "array_srf_32")
        .unwrap();
    let (doc, scene) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    enter(&mut app, "ArrayCrvOnSrf");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "SubCrv");
    enter(&mut app, "Enter");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.redo_label(), Some("Move"));
    enter(&mut app, "ArrayCrvOnSrf");
    enter(&mut app, "w0,0,0");
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: scene.target,
            index: 0,
            kind: viboceros_command::ComponentSelectionKind::BrepEdge,
        }],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    assert!(app.picking_array_surface(), "{:?}", app.command_log);
    assert!(app.accept_array_surface(scene.target, Some(0)));
    enter(&mut app, "Divide");
    enter(&mut app, "Enter");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before); // initial count 1 is rejected
    enter(&mut app, "2");
    assert_eq!(app.document.objects().count(), before.len() + 2);
    enter(&mut app, "Enter");
    assert!(app.active_command.is_some());
    enter(&mut app, "Multiple");
    enter(&mut app, "2");
    enter(&mut app, "-1");
    assert_eq!(app.document.objects().count(), before.len() + 2);
    enter(&mut app, "0");
    assert_eq!(app.document.objects().count(), before.len() + 2);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.undo_label(), Some("ArrayCrvOnSrf"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}
