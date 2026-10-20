use super::*;
use crate::curve_array_test_support as support;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn curve_array_native_getters_preview_and_history() {
    let mut app = test_app();
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (doc, scene) = support::setup(&value);
            app.document = doc;
            app.command_log.clear();
            let id = op["id"].as_str().unwrap();
            app.viewports[app.active_viewport].set_construction_plane(support::plane(&op));
            enter(&mut app, "ArrayCrv");
            let model = app.document.objects().cloned().collect::<Vec<_>>();
            for action in value["actions"].as_array().unwrap() {
                if let Some(text) = action.as_str() {
                    enter(&mut app, text);
                } else if action.is_array() {
                    enter(
                        &mut app,
                        &format!("w{}", support::token(support::point(action))),
                    );
                } else if let Some(n) = action.as_f64() {
                    enter(&mut app, &n.to_string());
                } else {
                    let index = action["curve"].as_u64().unwrap() as usize;
                    if index == 0 {
                        let (refs, _) = support::reference(&app.document, &scene, &op, &value);
                        assert!(
                            app.pick_curve_array_at(Some(scene.rail), refs.edge, refs.pick),
                            "{id} {:?}",
                            app.command_log
                        );
                    } else {
                        enter(&mut app, &format!("SelID {}", scene.sources[index - 1]));
                    }
                }
                if app
                    .curve_array_session
                    .as_ref()
                    .and_then(super::super::curve_array::Session::preview)
                    .is_some()
                {
                    assert_eq!(
                        app.document
                            .objects()
                            .map(|o| (o.id(), o.geometry().clone()))
                            .collect::<Vec<_>>(),
                        model
                            .iter()
                            .map(|o| (o.id(), o.geometry().clone()))
                            .collect::<Vec<_>>(),
                        "{id} preview mutated model"
                    );
                    assert!(app.document.undo_label().is_none());
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let mut max = support::check(&scene, &app.document, &op, &value, "after_script");
            if value["after_script"]["objects"].as_array().unwrap().len()
                > value["before"]["objects"].as_array().unwrap().len()
            {
                app.document.undo().unwrap();
                max = max.max(support::check(&scene, &app.document, &op, &value, "undo"));
                assert!(app.document.undo().unwrap().is_none());
                app.document.redo().unwrap();
                max = max.max(support::check(&scene, &app.document, &op, &value, "redo"));
            } else {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "CURVE_ARRAY_UI_AUDIT {}",
                serde_json::json!({"id":id,"maximum_error":max,"qualification":support::qualification(&op)})
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
fn curve_array_cancel_preview_and_stale_guide_preserve_redo() {
    let (op, value) = support::rows().remove(0);
    let (doc, scene) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ArrayCurve");
    app.pick_curve_array_at(Some(scene.rail), None, None);
    for input in ["Items", "8", "Orientation=Roadlike", "Distance", "2cm"] {
        enter(&mut app, input);
        assert!(app.command_input.is_empty());
    }
    assert_eq!(format!("{:?}", app.document), before);
    assert!(
        app.curve_array_session
            .as_ref()
            .unwrap()
            .preview()
            .is_some()
    );
    enter(&mut app, "Cancel");
    assert_eq!(format!("{:?}", app.document), before);
    assert_eq!(app.document.redo_label(), Some("Move"));
    enter(&mut app, "ArrayCrv");
    app.pick_curve_array_at(Some(scene.rail), None, None);
    app.document
        .set_objects_visibility([scene.rail], false)
        .unwrap();
    assert!(!app.validate_curve_array());
    assert!(app.picking_curve_array());
    assert!(
        app.curve_array_session
            .as_ref()
            .unwrap()
            .preview()
            .is_none()
    );
    enter(&mut app, "Cancel");
    let _ = op;
}

#[test]
fn curve_array_component_edges_and_source_changes_clear_transient_instances() {
    let (_, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["path"] == "Edge")
        .unwrap();
    let (doc, scene) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    enter(&mut app, "ArrayCrv");
    app.accept_component_click(crate::viewport::ComponentClick {
        picks: vec![crate::viewport::ComponentPick {
            object: scene.rail,
            index: 0,
            kind: viboceros_command::ComponentSelectionKind::BrepEdge,
        }],
        preselection: false,
        modifiers: egui::Modifiers::NONE,
    });
    assert!(
        app.curve_array_session
            .as_ref()
            .unwrap()
            .preview()
            .is_some(),
        "{:?}",
        app.command_log
    );
    assert!(app.document.undo_label().is_none());
    // An external edit invalidates the frozen sources rather than committing stale maps.
    app.commands
        .execute(&mut app.document, "Move 0,0,0 1,0,0")
        .unwrap();
    let edited = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(!app.validate_curve_array());
    assert!(app.curve_array_session.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), edited);
    assert_eq!(app.document.undo_label(), Some("Move"));
}
