use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_truncated_cone.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn truncated_cone_modes_caps_and_axes_match_native_geometry_sources_and_history() {
    let registry = CommandRegistry::with_builtins();
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
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
        assert_eq!(scene.document.undo_label(), Some("SubDTruncatedCone"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_TRUNCATED_CONE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_inputs_preserve_redo_and_all_command_preferences() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let prefs = registry.subd_truncated_cone_preferences();
    assert_eq!(
        (
            prefs.around_faces,
            prefs.vertical_faces,
            prefs.height,
            prefs.end_size
        ),
        (3, 8, 1., 1.)
    );
    let cylinder = registry.subd_cylinder_preferences();
    let cone = registry.subd_cone_preferences();
    let sphere = registry.subd_sphere_preferences();
    for args in [
        "0,0,0 -3 5 1",
        "0,0,0 0 5 1",
        "0,0,0 3 0 1",
        "0,0,0 3 5 -1",
        "VerticalFaces=1 0,0,0 3 5 1",
        "AroundFaces=2 0,0,0 3 5 1",
        "AroundFaces=100000 VerticalFaces=100000 0,0,0 3 5 1",
        "0,0,0 3 BothSides=Yes 5 1",
        "0,0,0 3 DirectionConstraint=None 5 1",
        "0,0,0 3 OrBaseAngle=30 1",
        "0,0,0 3 OrApexAngle=60 1",
        "AroundCurve 0,0,0 3 5 1",
        "0,0,0 Orientation 0,0,1 3 5 1",
        "0,0,0 3 5 Area 1",
        "0,0,0 3 5 1 trailing",
        "0,0,0 3 5",
    ] {
        let cmd = format!("SubDTruncatedCone {args}");
        assert!(registry.execute(&mut doc, &cmd).is_err(), "{cmd}");
        assert_eq!(format!("{doc:?}"), before, "{cmd}");
        assert_eq!(registry.subd_truncated_cone_preferences(), prefs);
        assert_eq!(registry.subd_cylinder_preferences(), cylinder);
        assert_eq!(registry.subd_cone_preferences(), cone);
        assert_eq!(registry.subd_sphere_preferences(), sphere);
    }
}
