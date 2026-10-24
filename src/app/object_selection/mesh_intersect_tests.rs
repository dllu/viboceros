#![cfg(feature = "native-smlib")]
use super::frozen_scene;
use super::mesh_boolean_tests::native as boolean;
use super::*;
use crate::app::tests::test_app;
use serde_json::json;
#[path = "../../../crates/viboceros-command/tests/support/mesh_intersect.rs"]
#[allow(dead_code)]
mod native;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn mesh_intersect_getters_replay_native_preferences_selection_and_history() {
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
        app.command_log.clear();
        enter(&mut app, native::name(&row));
        if native::menu(&row) {
            enter(&mut app, "CoplanarFaces");
            assert!(matches!(
                app.object_prompt.as_ref().unwrap().phase,
                ObjectPromptPhase::Choice(_)
            ));
            enter(&mut app, "Cancel");
        } else if app.object_prompt.is_some() {
            if let Some(arguments) = input["arguments"].as_str() {
                enter(&mut app, arguments);
            }
            if input["cancel"] == true {
                enter(&mut app, "Cancel");
            } else {
                app.select_prompt_objects(scene.ids.iter().copied(), SelectionMode::Add);
                enter(&mut app, "");
            }
        }
        assert!(
            app.object_prompt.is_none(),
            "{}: {:?}",
            row["id"],
            app.command_log
        );
        let error = app
            .command_log
            .iter()
            .find_map(|s| s.strip_prefix("Error: "))
            .map(str::to_owned);
        let success = !native::menu(&row) && input["cancel"] != true && error.is_none();
        let options = app
            .commands
            .object_selection_prompt("MeshIntersect")
            .unwrap()
            .unwrap();
        if let Some(mode) = boolean::expected_tolerance(value) {
            assert_eq!(options.choices[0].value, mode, "{}", row["id"]);
        }
        scene.doc = std::mem::take(&mut app.document);
        let after = boolean::check(&scene, value, "after");
        if native::name(&row) == "MeshIntersect" {
            assert!(boolean::sources_restored(&scene));
        }
        scene.doc.undo().unwrap();
        let undo = boolean::check(&scene, value, "undo");
        let sources_restored_on_undo = boolean::sources_restored(&scene);
        scene.doc.redo().unwrap();
        let redo = boolean::check(&scene, value, "redo");
        let measurement = json!({"id":row["id"],"success":success,"error":error,"after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":sources_restored_on_undo});
        println!("MESH_INTERSECT_GETTER {measurement}");
        if success != value["success"].as_bool().unwrap()
            || ["after", "undo", "redo"].iter().any(|stage| {
                measurement[stage]["geometry"] != true || measurement[stage]["metadata"] != true
            })
        {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        total += 1;
    }
    gaps.sort();
    assert_eq!(total, 84);
    assert_eq!(gaps, native::GAPS);
}
