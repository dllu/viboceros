#![cfg(feature = "native-smlib")]
use super::frozen_scene;
use super::mesh_boolean_tests::native as boolean;
use super::*;
use crate::app::tests::test_app;
use serde_json::json;
#[path = "../../../crates/viboceros-command/tests/support/mesh_split.rs"]
#[allow(dead_code)]
mod native;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn mesh_split_getters_replay_native_options_cutters_and_history() {
    let mut app = test_app();
    let mut family = serde_json::Value::Null;
    let mut gaps = Vec::new();
    let mut total = 0;
    for row in native::rows() {
        if family != row["family"] {
            app = test_app();
            family = row["family"].clone();
        }
        let value = &row["value"];
        let input = &value["input"];
        let mut scene = native::setup(&row);
        app.document = std::mem::take(&mut scene.doc);
        app.viewports[app.active_viewport]
            .set_construction_plane(native::context(&row).construction_plane);
        app.command_log.clear();
        enter(&mut app, native::name(&row));
        let ids = |set: &str| {
            input[set]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| scene.ids[i.as_u64().unwrap() as usize])
                .collect::<Vec<_>>()
        };
        if let Some(menu) = input["menu"].as_str() {
            enter(&mut app, menu);
            assert!(
                app.intersection_prompt
                    .as_ref()
                    .unwrap()
                    .boolean
                    .as_ref()
                    .unwrap()
                    .partition
                    .as_ref()
                    .unwrap()
                    .pending_choice
                    .is_some()
            );
            enter(&mut app, "Cancel");
        } else {
            if let Some(arguments) = input["arguments"].as_str().filter(|a| !a.is_empty()) {
                enter(&mut app, arguments);
            }
            if input["cancel"] == true {
                enter(&mut app, "Cancel");
            } else if native::name(&row) != "MeshSplit" {
                if app.object_prompt.is_some() {
                    app.select_prompt_objects(ids("first"), SelectionMode::Add);
                    enter(&mut app, "");
                }
            } else {
                if input["preselect"] == false {
                    app.select_intersection_prompt_objects(ids("first"), SelectionMode::Add);
                    enter(&mut app, "");
                }
                if let Some(arguments) = input["second_arguments"].as_str() {
                    for token in arguments.split_whitespace() {
                        enter(&mut app, token);
                    }
                }
                if native::cancelled(&row) {
                    enter(&mut app, "Cancel");
                } else {
                    app.select_intersection_prompt_objects(ids("second"), SelectionMode::Add);
                    enter(&mut app, "");
                }
            }
        }
        assert!(
            app.intersection_prompt.is_none()
                && app.object_prompt.is_none()
                && app.active_command.is_none(),
            "{}: {:?}",
            row["id"],
            app.command_log
        );
        let error = app
            .command_log
            .iter()
            .find_map(|s| s.strip_prefix("Error: "))
            .map(str::to_owned);
        let success = !native::cancelled(&row) && error.is_none();
        let options = app
            .commands
            .object_selection_prompt("MeshIntersect")
            .unwrap()
            .unwrap();
        if let Some(mode) = boolean::expected_tolerance(value) {
            assert_eq!(options.choices[0].value, mode, "{}", row["id"]);
        }
        scene.doc = std::mem::take(&mut app.document);
        let after = native::check(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = native::check(&scene, value, "undo");
        let ownership = boolean::sources_restored(&scene);
        scene.doc.redo().unwrap();
        let redo = native::check(&scene, value, "redo");
        let measured = json!({"id":row["id"],"success":success,"error":error,
            "after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":ownership});
        println!("MESH_SPLIT_GETTER {measured}");
        if success != value["success"].as_bool().unwrap()
            || ["after", "undo", "redo"].iter().any(|s| {
                measured[s]["metadata"] != true
                    || measured[s]["geometry"] != true
                    || measured[s]["ngons"] != true
            })
        {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        total += 1;
    }
    gaps.sort();
    assert_eq!(total, 65);
    assert_eq!(gaps, native::GAPS);
}
