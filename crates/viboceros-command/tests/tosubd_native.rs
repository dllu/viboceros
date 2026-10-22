#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod geometry_native;
#[allow(dead_code)]
#[path = "support/tosubd.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use viboceros_command::CommandRegistry;
use viboceros_document::SelectionMode;

#[test]
fn command_replays_native_conversion_attributes_selection_identity_and_history() {
    let mut count = 0;
    for row in native::rows() {
        eprintln!("checking command {}", row["id"]);
        let v = &row["value"];
        let mut scene = native::setup(v);
        let result = if v["input"]["cancel"] == true || v["input"]["cancel_selection"] == true {
            None
        } else {
            let command = format!("ToSubD {}", native::option_line(v));
            let faces = v["input"]["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.get("selected_faces").is_some());
            scene
                .doc
                .select_objects_direct(scene.ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            let command = if faces {
                let indices = v["input"]["sources"][0]["selected_faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| f.as_u64().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{command} Faces={indices}")
            } else {
                command
            };
            Some(CommandRegistry::with_builtins().execute(&mut scene.doc, &command))
        };
        if v["success"] == true {
            assert_eq!(
                result.unwrap().unwrap(),
                native::message(v),
                "{}",
                row["id"]
            );
            native::check(&scene, &v["after"]);
        } else {
            assert!(result.is_none_or(|r| r.is_err()), "{}", row["id"]);
        }
        scene.doc.clear_selection();
        let check = native::check(&scene, &v["after_script"]);
        let mutated = v["after_script"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o.get("subd").is_some());
        if mutated {
            assert_eq!(scene.doc.undo_label(), Some("ToSubD"));
            scene.doc.undo().unwrap();
            native::check(&scene, &v["undo"]);
            scene.doc.redo().unwrap();
            native::check(&scene, &v["redo"]);
        } else {
            assert_eq!(scene.doc.undo_label(), None);
        }
        println!(
            "TOSUBD_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true,"mutated":mutated})
        );
        count += 1;
    }
    assert_eq!(count, 201);
}
