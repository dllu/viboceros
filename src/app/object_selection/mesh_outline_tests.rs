use super::*;
use super::{frozen_scene, mesh_reference};
use crate::app::tests::test_app;
#[allow(dead_code)]
#[path = "../../../crates/viboceros-command/tests/support/mesh_outline.rs"]
mod native;
use serde_json::json;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn mesh_outline_getters_use_the_active_camera_and_match_native_lifecycle() {
    let mut gaps = Vec::new();
    let mut count = 0;
    for row in native::rows() {
        let v = &row["value"];
        eprintln!("Outline getter {}", row["id"]);
        let mut scene = native::setup(&row);
        let mut app = test_app();
        app.document = std::mem::take(&mut scene.doc);
        app.viewports[app.active_viewport]
            .restore_file_camera(&native::view_at(v, "before"))
            .unwrap();
        enter(&mut app, "MeshOutline");
        if v["input"]["preselect"] == false || v["success"] != true {
            assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
            assert_eq!(app.document.undo_label(), Some("Undo witness"));
            let selected = v["input"]["selected"].as_array().map_or_else(
                || scene.ids.clone(),
                |a| {
                    a.iter()
                        .map(|i| scene.ids[i.as_u64().unwrap() as usize])
                        .collect()
                },
            );
            if v["input"]["preselect"] == false {
                app.select_prompt_objects(selected, SelectionMode::Replace);
            }
            for (id, original) in scene.ids.iter().zip(&scene.sources) {
                assert!(
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(original)
                );
            }
            app.viewports[app.active_viewport]
                .restore_file_camera(&native::view(v))
                .unwrap();
            enter(
                &mut app,
                if v["input"]["cancel"] == true {
                    "Cancel"
                } else {
                    ""
                },
            );
        }
        assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
        scene.doc = std::mem::take(&mut app.document);
        let (after, mut matched, mut error) = native::check(&scene, v, "after");
        scene.doc.undo().unwrap();
        let (undo, good, e) = native::check(&scene, v, "undo");
        matched &= good;
        error = error.zip(e).map(|(a, b)| a.max(b));
        scene.doc.redo().unwrap();
        let (redo, good, e) = native::check(&scene, v, "redo");
        matched &= good;
        error = error.zip(e).map(|(a, b)| a.max(b));
        if !matched {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        println!(
            "MESH_OUTLINE_GETTER {}",
            json!({"id":row["id"],"after":after,"undo":undo,"redo":redo,"source_unchanged":true,"geometry":matched,"maximum_error":error})
        );
        count += 1;
    }
    assert_eq!(count, 39);
    gaps.sort();
    assert_eq!(
        gaps,
        [
            "outline_lifecycle_sphere",
            "outline_lifecycle_warped_perspective",
            "outline_quads_post"
        ]
    );
}
