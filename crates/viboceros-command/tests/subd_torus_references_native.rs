#[path = "support/subd_torus_surface.rs"]
mod torus_surface;
use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_cylinder_references.rs"]
#[allow(dead_code)]
mod cylinder_native;
#[path = "support/subd_torus_references.rs"]
#[allow(unused_imports)]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn torus_reference_commands_match_native_source_selection_geometry_and_grouped_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        let (command, post) = native::command(&mut scene, value);
        let result = if post {
            registry.execute_postselected(&mut scene.document, &command, CommandContext::default())
        } else {
            registry.execute(&mut scene.document, &command)
        };
        let declined = value["input"]["no_output"] == true;
        if declined {
            assert!(matches!(
                result,
                Err(viboceros_command::CommandError::Geometry(_))
            ));
            assert_eq!(scene.document.undo_label(), None);
        } else {
            result.unwrap_or_else(|e| panic!("{} {command}: {e}", row["id"]));
            assert_eq!(scene.document.undo_label(), Some("SubDTorus"));
        }
        let check = native::check(&scene, value, "after_script");
        if !declined {
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        }
        println!(
            "SUBD_TORUS_REFERENCE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true,"declined":declined})
        );
    }
}
