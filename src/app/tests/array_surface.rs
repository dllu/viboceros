use super::*;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn setup() -> (VibocerosApp, ObjectId, ObjectId) {
    let mut app = test_app();
    let source = app
        .document
        .add_geometry(Geometry::Point(point(1., 0., 0.)))
        .unwrap();
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(0., 0., 5.),
        point(10., 0., 5.),
        point(10., 10., 5.),
        point(0., 10., 5.),
    ])
    .unwrap();
    let target = app
        .document
        .add_geometry(Geometry::Brep(
            viboceros_geometry::Brep::try_surface_face(surface, app.document.tolerance()).unwrap(),
        ))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    (app, source, target)
}
#[test]
fn surface_array_preview_counts_mode_and_cancel_leave_redo_untouched() {
    let (mut app, source, target) = setup();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    enter(&mut app, "ArraySrf Preview=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    app.accept_surface_array_target(target, Some(0));
    enter(&mut app, "3");
    enter(&mut app, "2");
    assert_eq!(
        app.array_surface_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap()
            .transforms
            .len(),
        6
    );
    enter(&mut app, "UNumber=4");
    enter(&mut app, "Mode=Isocurve");
    assert_eq!(
        app.array_surface_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap()
            .transforms
            .len(),
        8
    );
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Cancel");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.redo_label(), Some("Move"));
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "ArraySurface Preview=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    app.accept_surface_array_target(target, None);
    enter(&mut app, "");
    enter(&mut app, "");
    enter(&mut app, "");
    assert_eq!(app.document.objects().count(), 10);
    assert!(app.document.is_selected(source));
    assert_eq!(app.document.undo_label(), Some("ArraySrf"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().count(), 2);
    assert!(app.document.undo().unwrap().is_none());
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 10);
}
#[test]
fn surface_array_native_prompts_preview_and_history_replay() {
    use crate::array_surface_test_support as support;
    let mut app = test_app();
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (doc, scene) = support::setup(&value);
            app.document = doc;
            let id = op["id"].as_str().unwrap();
            app.viewports[app.active_viewport].set_construction_plane(support::plane(&op));
            enter(&mut app, "ArraySrf");
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
                        assert!(
                            app.accept_surface_array_target(scene.target, None),
                            "{id} {:?}",
                            app.command_log
                        );
                    } else {
                        enter(&mut app, &format!("SelID {}", scene.sources[index - 1]));
                    }
                }
                if app
                    .array_surface_session
                    .as_ref()
                    .and_then(super::super::array_surface::Session::preview)
                    .is_some()
                {
                    assert_eq!(
                        app.document.objects().cloned().collect::<Vec<_>>(),
                        model,
                        "{id}: preview mutated model"
                    );
                    assert!(app.document.undo_label().is_none());
                }
            }
            assert!(app.active_command.is_none(), "{id} {:?}", app.command_log);
            let mut max = scene.check(&app.document, &value, "after_script", id);
            if value["after_script"]["objects"].as_array().unwrap().len()
                > value["before"]["objects"].as_array().unwrap().len()
            {
                app.document.undo().unwrap();
                max = max.max(scene.check(&app.document, &value, "undo", id));
                assert!(app.document.undo().unwrap().is_none());
                app.document.redo().unwrap();
                max = max.max(scene.check(&app.document, &value, "redo", id));
            } else {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "ARRAY_SURFACE_UI_AUDIT {}",
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
fn surface_array_invalid_getters_and_changed_targets_preserve_geometry_and_redo() {
    let (mut app, _, target) = setup();
    enter(&mut app, "Move 0,0,0 1,0,0");
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "ArraySrf Preview=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w0,0,0");
    assert!(!app.picking_surface_array());
    enter(&mut app, "Enter");
    assert!(app.picking_surface_array());
    app.accept_surface_array_target(target, None);
    enter(&mut app, "0");
    enter(&mut app, "2.5");
    enter(&mut app, "NaN");
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.redo_label(), Some("Move"));
    enter(&mut app, "2");
    enter(&mut app, "2");
    assert!(
        app.array_surface_session
            .as_ref()
            .unwrap()
            .preview()
            .is_some()
    );
    app.document
        .set_objects_visibility([target], false)
        .unwrap();
    let changed = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(!app.validate_surface_array());
    assert!(app.picking_surface_array());
    assert!(
        app.array_surface_session
            .as_ref()
            .unwrap()
            .preview()
            .is_none()
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), changed);
    enter(&mut app, "Cancel");
}
#[test]
fn accepted_surface_array_counts_clear_text_before_preview_acceptance() {
    let (mut app, _, target) = setup();
    enter(&mut app, "ArraySrf Preview=Yes");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "");
    app.accept_surface_array_target(target, None);
    enter(&mut app, "3");
    assert!(app.command_input.is_empty());
    enter(&mut app, "2");
    assert!(app.command_input.is_empty());
    assert_eq!(app.document.objects().count(), 2);
    app.run_command();
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 8);
}
