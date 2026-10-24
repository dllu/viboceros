//! Shared command replay driver, preserving each native family's option lifetime.
use super::native;
use serde_json::{Value, json};
use viboceros_command::{CommandContext, CommandRegistry};

pub fn replay(rows: Vec<Value>, prefix: &str) -> (Vec<Value>, Vec<Value>) {
    let mut registry = CommandRegistry::with_builtins();
    let mut family = serde_json::Value::Null;
    let mut measurements = vec![];
    let mut gaps = vec![];
    for row in rows {
        if row["family"] != family {
            registry = CommandRegistry::with_builtins();
            family = row["family"].clone();
        }
        let value = &row["value"];
        let input = &value["input"];
        if input["cancel"] == true
            || input["cancel_second"] == true
            || row["id"] == "boolean_lifecycle_union_one"
        {
            // Native options are accepted even if the getter is then cancelled.
            let scene = native::setup(&row);
            registry
                .accept_object_selection_input(&native::command(&scene, input))
                .unwrap();
            continue;
        }
        let mut scene = native::setup(&row);
        let before = native::check(&scene, value, "before");
        assert_eq!(before["metadata"], true, "{}", row["id"]);
        assert_eq!(before["geometry"], true, "{}", row["id"]);
        let (first, second) = native::sets(&scene, input);
        if input["preselect"] == false {
            native::pick(&mut scene, first.into_iter().chain(second));
        }
        let command = native::command(&scene, input);
        eprintln!("Mesh Boolean command {}: {command}", row["id"]);
        let result = if input["preselect"] == false {
            registry.execute_postselected(&mut scene.doc, &command, CommandContext::default())
        } else {
            registry.execute(&mut scene.doc, &command)
        };
        let success = result.is_ok();
        let prompt = registry
            .object_selection_prompt(command.split_whitespace().next().unwrap())
            .unwrap()
            .unwrap();
        if let Some(expected) = native::expected_tolerance(value) {
            assert_eq!(prompt.choices[0].value, expected, "{}", row["id"]);
        }
        let after = native::check(&scene, value, "after");
        scene.doc.undo().unwrap();
        let undo = native::check(&scene, value, "undo");
        let restored = native::sources_restored(&scene);
        assert!(restored, "{}", row["id"]);
        scene.doc.redo().unwrap();
        let redo = native::check(&scene, value, "redo");
        let matched = success == (value["success"] == true)
            && [&after, &undo, &redo]
                .iter()
                .all(|v| v["metadata"] == true && v["geometry"] == true);
        if !matched {
            gaps.push(row["id"].clone());
        }
        let row = json!({"id":row["id"],"success":success,"error":result.err().map(|e|e.to_string()),
            "after":after,"undo":undo,"redo":redo,"sources_restored_on_undo":restored});
        println!("{prefix} {row}");
        measurements.push(row);
    }
    gaps.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    eprintln!("{prefix}: {} commands; gaps {gaps:?}", measurements.len());
    (measurements, gaps)
}
