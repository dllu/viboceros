use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_sphere.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn sphere_styles_and_construction_modes_match_native_geometry_sources_and_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        let command = native::command(value, scene.source);
        registry
            .execute_in_context(
                &mut scene.document,
                &command,
                CommandContext {
                    construction_plane: native::plane(value),
                },
            )
            .unwrap_or_else(|e| panic!("{} {command}: {e}", row["id"]));
        let check = native::check(&scene, value, "after_script");
        assert_eq!(scene.document.undo_label(), Some("SubDSphere"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_SPHERE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_sphere_inputs_leave_the_model_redo_and_preferences_unchanged() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for cmd in [
        "SubDSphere 0,0,0 -3",
        "SubDSphere 0,0,0 0",
        "SubDSphere 0,0,0 NaN",
        "SubDSphere Style=Quad Subdivisions=7 0,0,0 3",
        "SubDSphere Style=Triangle Subdivisions=6 0,0,0 3",
        "SubDSphere AroundFaces=8 0,0,0 3",
        "SubDSphere Style=UV AroundFaces=100000 VerticalFaces=100000 0,0,0 3",
        "SubDSphere Style=Globe 0,0,0 3",
        "SubDSphere 0,0,0 3 trailing",
    ] {
        assert!(registry.execute(&mut doc, cmd).is_err(), "{cmd}");
        assert_eq!(format!("{doc:?}"), before, "{cmd}");
        assert_eq!(registry.subd_sphere_preferences(), Default::default());
    }
}
