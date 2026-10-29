use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn object_details_support_preselection_postselection_and_redo_without_model_edits() {
    for command in ["What", "List"] {
        for preselected in [false, true] {
            let mut app = test_app();
            let id = app
                .document
                .add_geometry(Geometry::Point(point(1., 2., 3.)))
                .unwrap();
            app.document
                .add_geometry(Geometry::Point(point(4., 5., 6.)))
                .unwrap();
            app.document.undo().unwrap();
            let original = app.document.object(id).unwrap().clone();
            if preselected {
                app.document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            enter(&mut app, command);
            if !preselected {
                assert!(app.object_prompt.is_some());
                assert_eq!(
                    app.viewport_object_filter(),
                    Some(viboceros_command::ObjectSelectionFilter::Any)
                );
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
                let before = format!("{:?}", app.document);
                enter(&mut app, "");
                assert_eq!(format!("{:?}", app.document), before);
            }
            assert!(app.command_line_idle());
            assert!(app.object_prompt.is_none());
            assert!(
                app.command_log
                    .iter()
                    .any(|s| s.contains(&format!("{command}: 1 object(s)"))
                        && s.contains(&id.to_string()))
            );
            assert_eq!(app.document.object(id), Some(&original));
            assert!(app.document.can_redo());
        }
    }
}

#[test]
fn object_details_cancel_does_not_consume_history_or_leave_a_prompt() {
    for command in ["What", "List"] {
        let mut app = test_app();
        app.document
            .add_geometry(Geometry::Point(point(1., 2., 3.)))
            .unwrap();
        let before = format!("{:?}", app.document);
        enter(&mut app, command);
        assert!(app.object_prompt.is_some());
        app.cancel_current_prompt_or_selection();
        assert!(app.command_line_idle());
        assert_eq!(format!("{:?}", app.document), before);
    }
}
