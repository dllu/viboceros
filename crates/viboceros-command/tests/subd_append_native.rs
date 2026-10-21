use viboceros_command::CommandRegistry;
#[path = "support/subd_append.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;

#[test]
fn append_retains_native_subd_geometry_detached_outputs_and_grouped_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        let command = native::command(&scene, value);
        registry
            .execute(&mut scene.document, &command)
            .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
        let check = native::check(&scene, value, "after_script");
        assert_eq!(scene.document.undo_label(), Some("3DFace"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_APPEND_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn native_limit_sector_normals_match_creased_smooth_and_nonmanifold_surfaces() {
    for row in native::normal_rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        let command = native::command(&scene, value);
        registry.execute(&mut scene.document, &command).unwrap();
        let geometry = native::check(&scene, value, "after_script");
        let normals = native::check_normals(&scene, value);
        println!(
            "SUBD_NORMAL_COMMAND {}",
            serde_json::json!({"id":row["id"],"geometry":geometry,"normals":normals})
        );
    }
}
