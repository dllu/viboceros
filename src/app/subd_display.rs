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
    fn interface_subd_sequences_match_native_api_states() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
        let query: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/interface_subd.json")).unwrap(),
        )
        .unwrap();
        let native: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("observations/interface_subd.json")).unwrap(),
        )
        .unwrap();
        for (op, expected) in query["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let mut app = test_app();
            app.grid_snap = op["grid_snap"].as_bool().unwrap();
            app.osnap = op["osnap"].as_bool().unwrap();
            app.smart_track = op["smart_track"].as_bool().unwrap();
            app.active_viewport = op["active_viewport"].as_u64().unwrap() as usize;
            app.document
                .set_subd_control_net(op["subd_control_net"].as_bool().unwrap());
            for (view, mode) in app
                .viewports
                .iter_mut()
                .zip(op["display_modes"].as_array().unwrap())
            {
                view.display_mode =
                    viboceros_command::interface::DisplayMode::parse(mode.as_str().unwrap())
                        .unwrap();
            }
            let snapshot = |app: &VibocerosApp| {
                serde_json::json!({
                "grid_snap":app.grid_snap,"osnap":app.osnap,"smart_track":app.smart_track,
                "active_viewport":app.active_viewport,"subd_control_net":app.document.subd_control_net(),
                "display_modes":app.viewports.iter().map(|v|v.display_mode.label()).collect::<Vec<_>>()})
            };
            let mut states = vec![snapshot(&app)];
            for command in op["commands"].as_array().unwrap() {
                enter(&mut app, command.as_str().unwrap());
                states.push(snapshot(&app));
            }
            let value = serde_json::json!({"states":states});
            assert_eq!(value, expected["value"], "{}", op["id"]);
            assert!(!app.document.can_undo());
            println!(
                "INTERFACE_SUBD_APP {}",
                serde_json::json!({"id":op["id"],"value":value})
            );
        }
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
