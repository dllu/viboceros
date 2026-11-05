//! Frozen native SubD edge commands through selection and live Zoom options.
use super::*;
use viboceros_oracle::subd_edge_analysis::{Fixture, snapshot, source};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn subd_edge_analysis_matches_native_picking_navigation_marking_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "subd_edge_analysis_discovery",
        "subd_edge_analysis_controls",
        "subd_edge_analysis_capabilities",
    ] {
        let request: serde_json::Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = super::oracle_observation::read(&base, family);
        for (op, row) in request["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            count += 1;
            let f: Fixture = serde_json::from_value(op.clone()).unwrap();
            let mut app = test_app();
            let id = app
                .document
                .add_geometry(source(&f, app.document.tolerance()).unwrap())
                .unwrap();
            if f.flat {
                enter(&mut app, "SubDDisplayToggle");
            }
            app.document.clear_history().unwrap();
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            if f.preselect {
                app.document
                    .select_objects_direct([id], SelectionMode::Replace)
                    .unwrap();
            }
            let storage = app.document.object(id).unwrap().geometry_snapshot().clone();
            let before = snapshot(&app.document, id).unwrap();
            enter(&mut app, &f.command);
            if !f.preselect {
                assert!(
                    app.object_prompt.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
                enter(&mut app, "");
            }
            assert!(app.object_prompt.is_none());
            if f.command == "ZoomNaked" {
                assert!(
                    app.edge_zoom_prompt.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
                for action in &f.actions {
                    enter(&mut app, action);
                }
                if f.finish == "Cancel" {
                    app.cancel_current_prompt_or_selection();
                } else {
                    enter(&mut app, "");
                }
                assert!(app.edge_zoom_prompt.is_none());
            } else {
                for action in &f.actions {
                    enter(&mut app, &format!("ShowEdges {action}"));
                }
            }
            assert!(
                storage.shares_storage_with(app.document.object(id).unwrap().geometry_snapshot())
            );
            let after = snapshot(&app.document, id).unwrap();
            let changed = !after["points"].as_array().unwrap().is_empty();
            let (undo, redo) = if changed {
                enter(&mut app, "Undo");
                let undo = snapshot(&app.document, id).unwrap();
                enter(&mut app, "Redo");
                (undo, snapshot(&app.document, id).unwrap())
            } else {
                assert!(app.document.can_redo());
                enter(&mut app, "Redo");
                let redo = snapshot(&app.document, id).unwrap();
                enter(&mut app, "Undo");
                (snapshot(&app.document, id).unwrap(), redo)
            };
            let actual = serde_json::json!({"before":before,"after":after,"undo":undo,"redo":redo});
            for phase in ["before", "after", "undo", "redo"] {
                super::subd_direction_support::compare(
                    &actual[phase],
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                );
            }
        }
    }
    assert_eq!(count, 54);
}
