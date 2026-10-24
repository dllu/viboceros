#![cfg(feature = "native-smlib")]
use super::*;
use super::{frozen_scene, mesh_reference};
use crate::app::tests::test_app;
#[allow(dead_code)]
#[path = "../../../crates/viboceros-command/tests/support/mesh_boolean.rs"]
mod native;
use serde_json::json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
fn options(app: &mut VibocerosApp, input: &str) {
    if !input.is_empty() {
        enter(app, input);
    }
}

fn replay(
    rows: Vec<serde_json::Value>,
    prefix: &str,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    let mut app = test_app();
    let mut family = serde_json::Value::Null;
    let mut gaps = vec![];
    let mut measurements = vec![];
    for row in rows {
        if row["family"] != family {
            app = test_app();
            family = row["family"].clone();
        }
        let value = &row["value"];
        let input = &value["input"];
        let mut scene = native::setup(&row);
        let (first, second) = native::sets(&scene, input);
        app.document = std::mem::take(&mut scene.doc);
        let operation = input["operation"].as_str().unwrap();
        let name = match operation {
            "union" => "MeshBooleanUnion",
            "intersection" => "MeshBooleanIntersection",
            "difference" => "MeshBooleanDifference",
            "split" => "MeshBooleanSplit",
            _ => unreachable!(),
        };
        app.command_log.clear();
        eprintln!("Mesh Boolean getter {}", row["id"]);
        enter(&mut app, name);
        if operation == "difference" && input["preselect"] == false {
            assert!(!app.command_log.back().unwrap().contains("DeleteInput="));
        }
        if app.object_prompt.is_some() || app.intersection_prompt.is_some() {
            options(&mut app, input["arguments"].as_str().unwrap_or(""));
            if input["cancel"] == true || row["id"] == "boolean_lifecycle_union_one" {
                // A single preselection is insufficient; native opens a new getter.
                enter(&mut app, "Cancel");
            } else if operation == "union" {
                for id in first {
                    app.select_prompt_objects([id], SelectionMode::Add);
                }
                enter(&mut app, "");
            } else {
                if input["preselect"] == false {
                    for id in first {
                        app.select_boolean_solids_objects([id], SelectionMode::Add);
                    }
                    enter(&mut app, "");
                    if operation == "difference" {
                        assert!(app.command_log.back().unwrap().contains("DeleteInput="));
                    }
                }
                options(&mut app, input["second_arguments"].as_str().unwrap_or(""));
                if input["cancel_second"] == true {
                    enter(&mut app, "Cancel");
                } else {
                    for id in second {
                        app.select_boolean_solids_objects([id], SelectionMode::Add);
                    }
                    enter(&mut app, "");
                }
            }
        }
        assert!(
            app.object_prompt.is_none() && app.intersection_prompt.is_none(),
            "{}: {:?}",
            row["id"],
            app.command_log
        );
        let prompt = app.commands.object_selection_prompt(name).unwrap().unwrap();
        if let Some(expected) = native::expected_tolerance(value) {
            assert_eq!(prompt.choices[0].value, expected, "{}", row["id"]);
        }
        let error = app
            .command_log
            .iter()
            .find_map(|line| line.strip_prefix("Error: "))
            .map(str::to_owned);
        let success = input["cancel"] != true
            && input["cancel_second"] != true
            && row["id"] != "boolean_lifecycle_union_one"
            && !app
                .command_log
                .iter()
                .any(|line| line.starts_with("Error:"));
        scene.doc = std::mem::take(&mut app.document);
        let after = native::check(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = native::check(&scene, value, "undo");
        let restored = native::sources_restored(&scene);
        assert!(restored, "{}", row["id"]);
        scene.doc.redo().unwrap();
        let redo = native::check(&scene, value, "redo");
        if success != (value["success"] == true)
            || [&after, &undo, &redo]
                .iter()
                .any(|r| r["metadata"] != true || r["geometry"] != true)
        {
            gaps.push(row["id"].clone());
        }
        let row = json!({"id":row["id"],"success":success,"error":error,"after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":restored});
        println!("{prefix} {row}");
        measurements.push(row);
    }
    gaps.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    eprintln!("{prefix}: {} getters; gaps {gaps:?}", measurements.len());
    (measurements, gaps)
}

#[test]
fn mesh_boolean_getters_replay_native_selection_options_geometry_and_history() {
    let (rows, gaps) = replay(native::rows(), "MESH_BOOLEAN_GETTER");
    assert_eq!(rows.len(), 85);
    assert_eq!(
        gaps,
        json!(native::COMPATIBILITY_GAPS)
            .as_array()
            .unwrap()
            .clone()
    );
}

#[test]
fn mesh_boolean_sets_getters_replay_native_selection_options_geometry_and_history() {
    let (rows, gaps) = replay(
        native::family_rows(&["sets", "split", "split_controls", "split_stages"]),
        "MESH_BOOLEAN_SETS_GETTER",
    );
    assert_eq!(rows.len(), 67);
    assert_eq!(
        gaps,
        json!(native::SET_COMPATIBILITY_GAPS)
            .as_array()
            .unwrap()
            .clone()
    );
}
