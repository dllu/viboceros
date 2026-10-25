//! Whole SubD preselection and real object/grip callbacks use the same locations.
use super::*;
use crate::app::tests::test_app;
use serde_json::json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn extract_points_whole_subd_getters_replay_native_history_and_attributes() {
    for row in native::whole_subd_rows() {
        let mut app = test_app();
        let (document, ids) = native::setup_whole_subd(&row);
        app.document = document;
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
        let whole = app.document.selected_object_ids().collect::<Vec<_>>();
        let grips = app
            .document
            .selected_control_points()
            .map(|(g, _)| g)
            .collect::<Vec<_>>();
        let picked = row["path"] == "picked";
        if picked {
            app.document.clear_selection();
        }
        enter(&mut app, &native::whole_subd_command(&row));
        if picked {
            assert!(
                app.object_prompt.is_some(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            for id in whole {
                assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
                    selection_click: Some(crate::viewport::SelectionClick {
                        object_id: Some(id),
                        mode: SelectionMode::Add,
                    }),
                    ..Default::default()
                }));
            }
            if !grips.is_empty() {
                assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
                    control_point_selection: Some(crate::viewport::ControlPointSelection {
                        picks: grips,
                        mode: SelectionMode::Replace
                    }),
                    ..Default::default()
                }));
            }
            enter(&mut app, "");
        }
        assert!(
            app.command_line_idle(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        assert_eq!(app.document.undo_label(), Some("ExtractPt"));
        let mut stages = Vec::new();
        for stage in ["after", "undo", "redo"] {
            if stage != "after" {
                enter(&mut app, if stage == "undo" { "Undo" } else { "Redo" });
            }
            native::compare(&app.document, &ids, &row["value"][stage]);
            for (id, original) in ids.iter().zip(&storage) {
                assert!(
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(original),
                    "{} {stage}",
                    row["id"]
                );
            }
            stages.push(native::snapshot(&app.document, &ids));
        }
        println!(
            "EXTRACT_POINTS_SUBD_GETTER {}",
            json!({"id":row["id"],"path":row["path"],
            "success":true,"source_storage_unchanged":true,"after":stages[0],"undo":stages[1],"redo":stages[2]})
        );
    }
}

#[test]
fn extract_points_subd_location_edits_are_local_and_cancel_preserves_redo() {
    use crate::app::tests::point;
    for row in native::whole_subd_rows().into_iter().filter(|r| {
        matches!(
            r["id"].as_str(),
            Some(
                "picked_whole_subd_irregular_flat_points"
                    | "picked_whole_subd_irregular_smooth_points"
            )
        )
    }) {
        let mut app = test_app();
        let (doc, ids) = native::setup_whole_subd(&row);
        app.document = doc;
        app.document.clear_selection();
        app.document
            .add_geometry(Geometry::Point(point(9., 9., 9.)))
            .unwrap();
        app.document.undo().unwrap();
        let redo = app.document.redo_label().map(str::to_owned);
        let source = app.document.object(ids[0]).unwrap().clone();
        enter(&mut app, "ExtractPt");
        enter(&mut app, "SubDLocation=ControlNet");
        app.select_prompt_objects(vec![ids[0]], SelectionMode::Add);
        app.cancel_current_prompt_or_selection();
        assert!(app.command_line_idle());
        assert_eq!(app.document.redo_label(), redo.as_deref());
        assert_eq!(app.document.objects().len(), 1);
        assert_eq!(app.document.object(ids[0]), Some(&source));
        enter(&mut app, "ExtractPt");
        assert!(
            app.object_prompt
                .as_ref()
                .unwrap()
                .description
                .command_line()
                .contains("SubDLocation=Surface")
        );
        if row["value"]["input"]["flat"] == true {
            enter(&mut app, "SubDLocation=ControlNet");
        }
        app.select_prompt_objects(vec![ids[0]], SelectionMode::Add);
        enter(&mut app, "");
        native::compare(&app.document, &ids, &row["value"]["after"]);
        assert!(!app.document.can_redo());
        assert_eq!(app.document.undo_label(), Some("ExtractPt"));
    }
}
