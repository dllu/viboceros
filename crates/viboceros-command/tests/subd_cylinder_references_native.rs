use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_cylinder_references.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn tangent_and_fitted_cylinder_bases_match_native_geometry_sources_and_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["success"] == true)
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
        assert_eq!(scene.document.undo_label(), Some("SubDCylinder"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_CYLINDER_REFERENCE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn degenerate_point_fit_accepts_height_then_declines_without_losing_redo() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["id"].as_str().unwrap().ends_with("_finish"))
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut scene.document, "Point 99,99,99")
            .unwrap();
        scene.document.undo().unwrap();
        let (command, post) = native::command(&mut scene, value);
        assert!(!post);
        assert!(registry.execute(&mut scene.document, &command).is_err());
        let check = native::check(&scene, value, "after_script");
        assert_eq!(scene.document.undo_label(), None);
        assert_eq!(scene.document.redo_label(), Some("Point"));
        assert_eq!(registry.subd_cylinder_preferences(), Default::default());
        println!(
            "SUBD_CYLINDER_REFERENCE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true,"declined":true})
        );
    }
}
