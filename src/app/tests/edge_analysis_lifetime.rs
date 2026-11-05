//! Native deletion/Undo evidence through app commands and simulated redraws.
use super::*;
use viboceros_oracle::edge_analysis_lifetime::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn stage(app: &VibocerosApp, ids: &[ObjectId], action: &str, runtime: bool) -> serde_json::Value {
    app.commands.edge_analysis_view(&app.document).unwrap();
    serde_json::json!({"action":action,"state":snapshot(&app.document,ids,runtime).unwrap(),"selection_states":ids.iter().map(|id|app.document.object_selection_state(*id) as u8).collect::<Vec<_>>()})
}

#[test]
fn edge_analysis_lifetime_matches_native_deletion_restoration_and_removed_sources() {
    replay("edge_analysis_lifetime", 13);
    replay("edge_analysis_lifetime_runtime", 29);
    replay("edge_analysis_lifetime_ui", 29);
    replay("edge_analysis_panel_selection", 14);
    replay("edge_analysis_modes", 11);
}
fn replay(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let request: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = super::oracle_observation::read(&base, family);
    let ops = request["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(ops.len(), count);
    assert_eq!(ops.len(), rows.len());
    for (op, row) in ops.iter().zip(rows) {
        let f: Fixture = serde_json::from_value(op.clone()).unwrap();
        let mut app = test_app();
        let (doc, ids) = setup(&f, app.document.tolerance()).unwrap();
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
        app.document
            .select_objects_direct(f.analyzed.iter().map(|&i| ids[i]), SelectionMode::Replace)
            .unwrap();
        let before = snapshot(&app.document, &ids, f.runtime).unwrap();
        enter(&mut app, "ShowEdges");
        enter(
            &mut app,
            &format!(
                "ShowEdges Show={}",
                f.initial_mode.as_deref().unwrap_or("Naked")
            ),
        );
        let mut stages = vec![stage(&app, &ids, "Start", f.runtime)];
        if !f.removed.is_empty() {
            app.document
                .select_objects_direct(f.removed.iter().map(|&i| ids[i]), SelectionMode::Replace)
                .unwrap();
            enter(&mut app, "ShowEdges Remove");
            stages.push(stage(&app, &ids, "Remove", f.runtime));
        }
        app.document
            .select_objects_direct(f.targets.iter().map(|&i| ids[i]), SelectionMode::Replace)
            .unwrap();
        if f.edit == "Select" {
            stages.push(stage(&app, &ids, "Select", f.runtime));
            let modes = if f.mode_sequence.is_empty() {
                vec![f.panel_mode.as_deref().unwrap()]
            } else {
                f.mode_sequence.iter().map(String::as_str).collect()
            };
            for (i, mode) in modes.into_iter().enumerate() {
                if i > 0 {
                    app.document
                        .select_objects_direct(
                            f.targets.iter().map(|&i| ids[i]),
                            SelectionMode::Replace,
                        )
                        .unwrap();
                    stages.push(stage(&app, &ids, &format!("Select{i}"), f.runtime));
                }
                if mode != "Zoom" {
                    enter(&mut app, &format!("ShowEdges Show={mode}"));
                    let action = if f.mode_sequence.is_empty() {
                        "Mode".into()
                    } else {
                        format!("Mode{i}")
                    };
                    stages.push(stage(&app, &ids, &action, f.runtime));
                }
            }
        } else {
            let input = if f.edit == "Move" {
                "Move 0,0,0 7,2,1"
            } else {
                f.edit.as_str()
            };
            let undo = format!("Undo{}", f.edit);
            let redo = format!("Redo{}", f.edit);
            for (action, input) in [
                (f.edit.as_str(), input),
                (undo.as_str(), "Undo"),
                (redo.as_str(), "Redo"),
                ("Restore", "Undo"),
            ] {
                enter(&mut app, input);
                stages.push(stage(&app, &ids, action, f.runtime));
            }
        }
        for (id, original) in ids.iter().zip(&storage) {
            assert!(
                original.shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
            );
        }
        if !f.skip_zoom {
            app.start_edge_analysis_zoom();
        }
        if app.edge_zoom_prompt.is_some() {
            enter(&mut app, "All");
            enter(&mut app, "Mark");
            enter(&mut app, "");
        }
        stages.push(stage(&app, &ids, "Zoom", f.runtime));
        let after = snapshot(&app.document, &ids, f.runtime).unwrap();
        if !after["points"].as_array().unwrap().is_empty() {
            assert_eq!(app.document.undo_label(), Some("ZoomNaked"));
            enter(&mut app, "Undo");
            stages.push(stage(&app, &ids, "UndoMark", f.runtime));
            enter(&mut app, "Redo");
            stages.push(stage(&app, &ids, "RedoMark", f.runtime));
        }
        for (phase, a) in [("before", before), ("after", after)] {
            super::edge_analysis_mixed_support::compare(
                &a,
                &row["value"][phase],
                &format!("{}/{phase}", op["id"]),
            );
        }
        let native_stages = row["value"]["stages"].as_array().unwrap();
        assert_eq!(stages.len(), native_stages.len(), "{}", op["id"]);
        for (a, b) in stages.iter().zip(native_stages) {
            assert_eq!(a["action"], b["action"]);
            if b.get("selection_states").is_some() {
                assert_eq!(
                    a["selection_states"], b["selection_states"],
                    "{} {}",
                    op["id"], a["action"]
                );
            }
            super::edge_analysis_mixed_support::compare(
                &a["state"],
                &b["state"],
                &format!("{}/{}", op["id"], a["action"]),
            );
        }
    }
}
