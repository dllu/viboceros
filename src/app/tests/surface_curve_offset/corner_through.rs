use super::*;

#[test]
fn corner_point_getters_keep_previews_failures_and_history_in_sync_with_native() {
    for (op, value) in native::corner_through_rows() {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        enter(&mut app, "OffsetCrvOnSrf");
        for action in op["actions"].as_array().unwrap() {
            if action["curve"].as_u64().is_some() {
                app.pick_surface_offset_face(Some(scene.target), Some(0), None);
            } else if let Some(text) = action.as_str() {
                enter(&mut app, text);
            } else {
                let [x, y, z]: [f64; 3] = serde_json::from_value(action.clone()).unwrap();
                let point = point(x, y, z);
                let before = format!("{:?}", app.document);
                app.update_surface_offset_hover(point);
                let pointer = app
                    .surface_offset_session
                    .as_ref()
                    .unwrap()
                    .preview_curves()
                    .as_ptr();
                app.update_surface_offset_hover(point);
                assert_eq!(
                    pointer,
                    app.surface_offset_session
                        .as_ref()
                        .unwrap()
                        .preview_curves()
                        .as_ptr()
                );
                assert_eq!(format!("{:?}", app.document), before);
                enter(&mut app, &format!("w{x},{y},{z}"));
                assert!(
                    app.active_command.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
            }
        }
        assert!(
            app.active_command.is_none(),
            "{}: {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        let count = value["after_script"].as_array().unwrap().len() - 2;
        if count > 0 {
            app.document.undo().unwrap();
            maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
            assert!(app.document.undo().unwrap().is_none());
            app.document.redo().unwrap();
            maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        } else {
            assert!(app.document.undo_label().is_none());
        }
        println!(
            "SURFACE_OFFSET_CORNER_POINT_GETTER {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":count})
        );
        assert!(maximum < 1e-6, "{}: {maximum:e}", op["id"]);
    }
}

#[test]
fn selected_corner_point_getters_retain_resolved_offsets_and_finish_with_native_history() {
    for (op, value) in native::corner_select_rows() {
        let mut app = test_app();
        let (doc, scene) = native::setup(&op, &value);
        app.document = doc;
        enter(&mut app, "OffsetCrvOnSrf");
        let initial = app.document.objects().count();
        for action in op["actions"].as_array().unwrap() {
            if let Some(index) = action["curve"].as_u64() {
                if index == 1 {
                    app.pick_surface_offset_face(Some(scene.target), Some(0), None);
                } else {
                    let id = scene.references[index as usize - 2];
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Replace,
                    });
                    assert_eq!(app.document.objects().count(), initial);
                    assert!(app.document.undo_label().is_none());
                    assert_eq!(app.document.selected_object_count(), 0);
                }
            } else {
                enter(&mut app, action.as_str().unwrap());
            }
        }
        assert!(
            app.active_command.is_none(),
            "{}: {:?}",
            op["id"],
            app.command_log
        );
        let mut maximum = native::check(&app.document, &scene, &value, "after_script");
        app.document.undo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "undo"));
        assert!(app.document.undo().unwrap().is_none());
        app.document.redo().unwrap();
        maximum = maximum.max(native::check(&app.document, &scene, &value, "redo"));
        println!(
            "SURFACE_OFFSET_CORNER_SELECT_GETTER {}",
            serde_json::json!({"id":op["id"],"maximum_bidirectional_sample_error":maximum,"qualified":maximum<1e-6,"outputs":value["after_script"].as_array().unwrap().len()-initial})
        );
        assert!(maximum < 1e-6, "{}: {maximum:e}", op["id"]);
    }
}
