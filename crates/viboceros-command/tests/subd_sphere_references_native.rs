use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_cylinder_references.rs"]
#[allow(dead_code)]
mod cylinder_native;
#[path = "support/subd_sphere_references.rs"]
#[allow(unused_imports)]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn sphere_references_match_native_sources_selection_geometry_and_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        native::check(&scene, value, "before");
        let (command, post) = native::command(&mut scene, value);
        let result = if post {
            registry.execute_postselected(&mut scene.document, &command, CommandContext::default())
        } else {
            registry.execute(&mut scene.document, &command)
        };
        result.unwrap_or_else(|e| panic!("{} {command}: {e}", row["id"]));
        let check = native::check(&scene, value, "after_script");
        if value["input"]["no_output"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDSphere"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_SPHERE_REFERENCE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
