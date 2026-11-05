//! Mixed diagnostics through the object getter, live Zoom input and panel entry.
use super::*;
use viboceros_oracle::edge_analysis_mixed::{Fixture, setup, snapshot};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn mixed_edge_diagnostics_match_native_selection_order_membership_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "edge_analysis_mixed",
        "edge_analysis_mixed_ui",
        "edge_analysis_mixed_selection",
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
            enter(&mut app, "Point 40,50,60");
            enter(&mut app, "Undo");
            if f.preselect {
                let selected = if f.membership && !f.panel_only {
                    &ids[..1]
                } else {
                    &ids
                };
                app.document
                    .select_objects_direct(selected.iter().copied(), SelectionMode::Replace)
                    .unwrap();
            }
            let before = snapshot(&app.document, &ids, &f).unwrap();
            enter(&mut app, &f.command);
            if !f.preselect {
                assert!(
                    app.object_prompt.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
                for id in &ids {
                    app.apply_selection_click(SelectionClick {
                        object_id: Some(*id),
                        mode: SelectionMode::Add,
                    });
                }
                enter(&mut app, "");
            }
            assert!(app.object_prompt.is_none());
            let mut stages = Vec::new();
            if f.membership {
                stages.push(serde_json::json!({"action":"Start","state":snapshot(&app.document,&ids,&f).unwrap()}));
                if !f.panel_only {
                    let batches = if f.sequential {
                        ids[1..].chunks(1).collect::<Vec<_>>()
                    } else {
                        vec![&ids[1..]]
                    };
                    for (i, batch) in batches.into_iter().enumerate() {
                        let select = if f.sequential {
                            format!("Select{}", i + 1)
                        } else {
                            "SelectRest".into()
                        };
                        let add = if f.sequential {
                            format!("Add{}", i + 1)
                        } else {
                            "Add".into()
                        };
                        app.document
                            .select_objects_direct(batch.iter().copied(), SelectionMode::Replace)
                            .unwrap();
                        stages.push(serde_json::json!({"action":select,"state":snapshot(&app.document,&ids,&f).unwrap()}));
                        enter(&mut app, "ShowEdges Add");
                        stages.push(serde_json::json!({"action":add,"state":snapshot(&app.document,&ids,&f).unwrap()}));
                    }
                    assert_eq!(
                        app.commands
                            .edge_analysis_view(&app.document)
                            .unwrap()
                            .unwrap()
                            .sources,
                        3
                    );
                    app.document
                        .select_objects_direct([ids[0]], SelectionMode::Replace)
                        .unwrap();
                    stages.push(serde_json::json!({"action":"SelectFirst","state":snapshot(&app.document,&ids,&f).unwrap()}));
                    enter(&mut app, "ShowEdges Remove");
                    stages.push(serde_json::json!({"action":"Remove","state":snapshot(&app.document,&ids,&f).unwrap()}));
                }
                enter(&mut app, "ShowEdges Show=Naked");
                if f.mode_trace {
                    stages.push(serde_json::json!({"action":"Mode","state":snapshot(&app.document,&ids,&f).unwrap()}));
                }
                app.start_edge_analysis_zoom();
                assert!(
                    app.commands
                        .edge_zoom_view(&app.document)
                        .unwrap()
                        .unwrap()
                        .focus_all
                );
                assert!(app.edge_zoom_prompt.is_some());
                enter(&mut app, "All");
                enter(&mut app, "Mark");
                enter(&mut app, "");
                stages.push(serde_json::json!({"action":"Zoom","state":snapshot(&app.document,&ids,&f).unwrap()}));
                assert_eq!(
                    app.commands
                        .edge_analysis_view(&app.document)
                        .unwrap()
                        .unwrap()
                        .sources,
                    if f.panel_only { 3 } else { 2 }
                );
            } else if f.command == "ZoomNaked" {
                assert!(
                    app.edge_zoom_prompt.is_some(),
                    "{}: {:?}",
                    op["id"],
                    app.command_log
                );
                for action in &f.actions {
                    enter(&mut app, action);
                }
                enter(&mut app, "");
            }
            assert!(app.edge_zoom_prompt.is_none());
            for (id, original) in ids.iter().zip(storage) {
                assert!(
                    original
                        .shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
                );
            }
            let after = snapshot(&app.document, &ids, &f).unwrap();
            let (undo, redo) = if after["points"].as_array().unwrap().is_empty() {
                assert!(app.document.can_redo());
                enter(&mut app, "Redo");
                let redo = snapshot(&app.document, &ids, &f).unwrap();
                enter(&mut app, "Undo");
                (snapshot(&app.document, &ids, &f).unwrap(), redo)
            } else {
                assert_eq!(app.document.undo_label(), Some("ZoomNaked"));
                enter(&mut app, "Undo");
                let undo = snapshot(&app.document, &ids, &f).unwrap();
                enter(&mut app, "Redo");
                (undo, snapshot(&app.document, &ids, &f).unwrap())
            };
            let actual = serde_json::json!({"before":before,"after":after,"undo":undo,"redo":redo});
            for phase in ["before", "after", "undo", "redo"] {
                super::edge_analysis_mixed_support::compare(
                    &actual[phase],
                    &row["value"][phase],
                    &format!("{}/{phase}", op["id"]),
                );
            }
            if f.membership {
                let native = row["value"]["stages"].as_array().unwrap();
                assert_eq!(stages.len(), native.len());
                for (a, b) in stages.iter().zip(native) {
                    assert_eq!(a["action"], b["action"]);
                    super::edge_analysis_mixed_support::compare(
                        &a["state"],
                        &b["state"],
                        &format!("{}/{}", op["id"], a["action"]),
                    );
                }
            }
        }
    }
    assert_eq!(count, 92);
}

#[test]
fn panel_zoom_is_disabled_in_all_mode_without_clearing_selection_or_redo() {
    let mut app = test_app();
    enter(&mut app, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0");
    enter(&mut app, "SelAll");
    enter(&mut app, "ShowEdges Show=All");
    enter(&mut app, "Point 40,50,60");
    enter(&mut app, "Undo");
    enter(&mut app, "SelAll");
    app.start_edge_analysis_zoom();
    assert!(app.edge_zoom_prompt.is_none());
    assert!(
        app.commands
            .edge_zoom_view(&app.document)
            .unwrap()
            .is_none()
    );
    assert_eq!(app.document.selected_objects().count(), 1);
    assert!(app.document.can_redo());
}
