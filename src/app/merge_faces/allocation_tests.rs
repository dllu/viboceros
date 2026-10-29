use super::face_properties_tests::setup;
use super::tests::support;
use super::*;
use crate::viewport::ComponentClick;
use serde_json::{Value, json};
use viboceros_command::ComponentSelectionKind;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command_input();
}
fn pick(app: &mut VibocerosApp, id: ObjectId, index: usize) {
    app.accept_component_click(ComponentClick {
        picks: vec![ComponentPick {
            object: id,
            kind: ComponentSelectionKind::SubDFace,
            index,
        }],
        preselection: true,
        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
    });
}
fn selections(app: &mut VibocerosApp, id: ObjectId) -> Vec<Value> {
    // The UI performs this pruning once per frame; command sequences must not
    // resurrect old components after their geometry is replaced by history.
    let picks = app.component_selection.valid_picks(&app.document);
    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
        panic!("owned SubD")
    };
    let mut out = picks
        .iter()
        .filter(|p| p.object == id)
        .map(|p| match p.kind {
            ComponentSelectionKind::SubDFace => json!(["face", s.face_ids()[p.index]]),
            ComponentSelectionKind::SubDEdge => json!(["edge", s.edge_ids()[p.index]]),
            ComponentSelectionKind::SubDVertex => json!(["vertex", s.vertex_ids()[p.index]]),
            _ => panic!("owned SubD pick"),
        })
        .collect::<Vec<_>>();
    out.sort_by_key(Value::to_string);
    out
}
fn native_selections(row: &Value) -> Vec<Value> {
    let mut picks = Vec::new();
    for id in row["selected_faces"].as_array().unwrap() {
        picks.push(json!(["face", id]));
    }
    for (kind, key, ids) in [
        ("edge", "selected_edges", "edge_ids"),
        ("vertex", "selected_vertices", "vertex_ids"),
    ] {
        for i in row[key].as_array().unwrap() {
            picks.push(json!([
                kind,
                row["subd"][ids][i.as_u64().unwrap() as usize]
            ]));
        }
    }
    picks.sort_by_key(Value::to_string);
    picks
}
fn check(app: &mut VibocerosApp, id: ObjectId, rows: &Value, name: &str) {
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("subd").is_some())
        .unwrap();
    assert_eq!(
        app.document.is_selected(id),
        row["selected"].as_bool().unwrap(),
        "{name} whole"
    );
    assert_eq!(
        selections(app, id),
        native_selections(row),
        "{name} components"
    );
}

#[test]
fn native_merge_then_global_and_local_subdivide_replay_selection_and_ids() {
    let cases: Value = serde_json::from_str(
        &std::fs::read_to_string(
            support::root().join("tools/rhino_oracle/fixtures/subd_merge_allocation_recipes.json"),
        )
        .unwrap(),
    )
    .unwrap();
    for family in [
        "subd_merge_allocation_live",
        "subd_merge_allocation_keyboard",
    ] {
        let record: Value = serde_json::from_str(
            &std::fs::read_to_string(
                support::root().join(format!("tools/rhino_oracle/observations/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let (mut app, id, original) = setup(&v["source_3dm"]);
            for f in cases[name]["faces"].as_array().unwrap() {
                pick(&mut app, id, f.as_u64().unwrap() as usize);
            }
            enter(&mut app, "MergeFaces");
            check(&mut app, id, &v["merged"], name);
            app.component_selection.clear();
            if let Some(face) = v["local_face"].as_u64() {
                let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
                    panic!("owned SubD")
                };
                let index = s.face_ids().iter().position(|&i| i as u64 == face).unwrap();
                pick(&mut app, id, index);
            } else {
                app.document
                    .select_objects_direct([id], viboceros_document::SelectionMode::Replace)
                    .unwrap();
            }
            enter(&mut app, "SubDivide");
            check(&mut app, id, &v["after"], &format!("{name}/after"));
            let expected = support::models(&v["after_3dm"]);
            support::compare(
                &support::geometry_value(app.document.object(id).unwrap().geometry()),
                &support::geometry_value(&expected["Input 0"].1),
                name,
            );
            for (i, rows) in v["undone"].as_array().unwrap().iter().enumerate() {
                enter(&mut app, "Undo");
                check(&mut app, id, rows, &format!("{name}/undo-{i}"));
            }
            assert_eq!(app.document.object(id).unwrap().geometry(), &original);
            for (i, rows) in v["redone"].as_array().unwrap().iter().enumerate() {
                enter(&mut app, "Redo");
                check(&mut app, id, rows, &format!("{name}/redo-{i}"));
            }
        }
    }
}

#[test]
fn native_merge_result_selection_changes_before_undo_control_redo_faces() {
    for family in ["subd_result_history", "subd_result_history_reselect"] {
        let record: Value = serde_json::from_str(
            &std::fs::read_to_string(
                support::root().join(format!("tools/rhino_oracle/observations/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in record["results"].as_array().unwrap() {
            let name = row["id"].as_str().unwrap();
            let v = &row["value"];
            let (mut app, id, _) = setup(&v["source_3dm"]);
            pick(&mut app, id, 0);
            pick(&mut app, id, 1);
            enter(&mut app, "MergeFaces");
            check(&mut app, id, &v["after_command"], name);
            if name != "keep" {
                app.component_selection.clear();
            }
            if name == "whole" {
                app.document
                    .select_objects_direct([id], viboceros_document::SelectionMode::Replace)
                    .unwrap();
            }
            let native = v["after_selection"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r.get("subd").is_some())
                .unwrap();
            if name != "keep" {
                for item in native_selections(native) {
                    let Geometry::SubD(s) = app.document.object(id).unwrap().geometry() else {
                        panic!("owned SubD")
                    };
                    let (kind, ids) = match item[0].as_str().unwrap() {
                        "face" => (ComponentSelectionKind::SubDFace, s.face_ids()),
                        "edge" => (ComponentSelectionKind::SubDEdge, s.edge_ids()),
                        "vertex" => (ComponentSelectionKind::SubDVertex, s.vertex_ids()),
                        _ => unreachable!(),
                    };
                    let index = ids
                        .binary_search(&(item[1].as_u64().unwrap() as u32))
                        .unwrap();
                    app.accept_component_click(ComponentClick {
                        picks: vec![ComponentPick {
                            object: id,
                            kind,
                            index,
                        }],
                        preselection: true,
                        modifiers: egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                    });
                }
            }
            check(&mut app, id, &v["after_selection"], name);
            enter(&mut app, "Undo");
            check(&mut app, id, &v["undone"], &format!("{name}/undo"));
            enter(&mut app, "Redo");
            check(&mut app, id, &v["redone"], &format!("{name}/redo"));
            if v.get("reselected").is_some() {
                app.component_selection.clear();
                app.document.clear_selection();
                pick(&mut app, id, 0);
                check(
                    &mut app,
                    id,
                    &v["reselected"],
                    &format!("{name}/reselected"),
                );
                enter(&mut app, "Undo");
                check(
                    &mut app,
                    id,
                    &v["reselected_undo"],
                    &format!("{name}/reselected-undo"),
                );
                enter(&mut app, "Redo");
                check(
                    &mut app,
                    id,
                    &v["reselected_redo"],
                    &format!("{name}/reselected-redo"),
                );
            }
        }
    }
}

#[test]
fn toggling_a_result_face_off_and_on_does_not_reenable_automatic_redo() {
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(
            support::root()
                .join("tools/rhino_oracle/observations/subd_result_history_reselect.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let v = &record["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "keep")
        .unwrap()["value"];
    let (mut app, id, _) = setup(&v["source_3dm"]);
    pick(&mut app, id, 0);
    pick(&mut app, id, 1);
    enter(&mut app, "MergeFaces");
    check(&mut app, id, &v["after_command"], "merged");
    pick(&mut app, id, 0); // Ctrl+Shift preselection toggles the result off.
    assert!(selections(&mut app, id).is_empty());
    pick(&mut app, id, 0);
    check(&mut app, id, &v["reselected"], "explicitly reselected");
    enter(&mut app, "Undo");
    enter(&mut app, "Redo");
    check(&mut app, id, &v["reselected_redo"], "redo");
}
