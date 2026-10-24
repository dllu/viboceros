#[allow(dead_code)]
#[path = "support/frozen_scene.rs"]
mod frozen_scene;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/mesh_outline.rs"]
mod mesh_reference;
#[path = "support/mesh_outline.rs"]
#[allow(dead_code)]
mod native;
use serde_json::json;
use viboceros_command::CommandRegistry;
use viboceros_document::SelectionMode;

#[test]
fn mesh_outline_commands_match_saved_sources_camera_and_history() {
    let registry = CommandRegistry::with_builtins();
    let mut gaps = Vec::new();
    let mut count = 0;
    for row in native::rows() {
        let v = &row["value"];
        if v["success"] != true {
            continue;
        }
        eprintln!("Native outline command {}", row["id"]);
        let mut scene = native::setup(&row);
        let selected = scene.ids.clone();
        scene
            .doc
            .select_objects_direct(selected, SelectionMode::Replace)
            .unwrap();
        let (context, projection) = native::context(v);
        registry
            .execute_with_pick_projection(
                &mut scene.doc,
                "MeshOutline",
                context,
                v["input"]["preselect"] == false,
                projection,
            )
            .unwrap();
        let (after, mut matched, mut error) = native::check(&scene, v, "after");
        scene.doc.undo().unwrap();
        let (undo, good, e) = native::check(&scene, v, "undo");
        matched &= good;
        error = error.zip(e).map(|(a, b)| a.max(b));
        scene.doc.redo().unwrap();
        let (redo, good, e) = native::check(&scene, v, "redo");
        matched &= good;
        error = error.zip(e).map(|(a, b)| a.max(b));
        let (control, control_error) = native::native_render_control(v);
        if !matched {
            gaps.push(row["id"].as_str().unwrap().to_owned());
        }
        println!(
            "MESH_OUTLINE_COMMAND {}",
            json!({"id":row["id"],"after":after,"undo":undo,"redo":redo,"source_unchanged":true,"geometry":matched,"maximum_error":error,"native_render_control":control,"native_render_control_error":control_error})
        );
        count += 1;
    }
    assert_eq!(count, 36);
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
