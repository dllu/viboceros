//! Display toggles nest inside selection without committing the pending command.
#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::app::{extract_points::native, tests::test_app};
    fn enter(app: &mut VibocerosApp, input: &str) {
        app.command_input = input.into();
        app.run_command();
    }
    #[test]
    fn subd_display_toggle_preserves_a_pending_point_direction() {
        let mut app = test_app();
        enter(&mut app, "Line");
        enter(&mut app, "0,0,0");
        let pending = app.active_command;
        enter(&mut app, "'_SubDDisplayToggle");
        assert!(app.document.subd_control_net());
        assert_eq!(app.active_command, pending);
        enter(&mut app, "1,2,3");
        assert!(app.command_line_idle());
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.undo_label(), Some("Line"));
    }
    #[test]
    fn subd_display_toggles_nested_in_native_extraction_getters() {
        for row in native::display_rows() {
            let (doc, ids) = native::setup(&row);
            let mut app = test_app();
            app.document = doc;
            let storage = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            let flat = row["value"]["input"]["flat"].as_bool().unwrap();
            app.document.set_subd_control_net(!flat);
            let grips = app
                .document
                .selected_control_points()
                .map(|(g, _)| g)
                .collect::<Vec<_>>();
            app.document.clear_selection();
            enter(&mut app, &native::command(&row));
            app.select_prompt_objects(ids.clone(), SelectionMode::Add);
            if !grips.is_empty() {
                app.handle_viewport_action(crate::viewport::ViewportOutput {
                    control_point_selection: Some(crate::viewport::ControlPointSelection {
                        picks: grips,
                        mode: SelectionMode::Add,
                    }),
                    ..Default::default()
                });
            }
            let before = native::snapshot(&app.document, &ids);
            enter(&mut app, "'_SubDDisplayToggle");
            assert!(
                app.object_prompt.is_some(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            assert_eq!(app.document.subd_control_net(), flat);
            assert_eq!(native::snapshot(&app.document, &ids), before);
            enter(&mut app, "");
            let mut stages = Vec::new();
            let mut modes = Vec::new();
            for stage in ["after", "undo", "redo"] {
                if stage != "after" {
                    enter(&mut app, if stage == "undo" { "Undo" } else { "Redo" });
                }
                for (id, source) in ids.iter().zip(&storage) {
                    assert!(
                        app.document
                            .object(*id)
                            .unwrap()
                            .geometry_snapshot()
                            .shares_storage_with(source)
                    );
                }
                native::compare(&app.document, &ids, &row["value"][stage]);
                stages.push(native::snapshot(&app.document, &ids));
                modes.push(app.document.subd_control_net());
            }
            println!(
                "SUBD_DISPLAY_GETTER {}",
                serde_json::json!({"id":row["id"],"success":true,
                "toggle_preserved":true,"source_storage_unchanged":true,"modes":modes,"after":stages[0],"undo":stages[1],"redo":stages[2]})
            );
        }
    }
}
