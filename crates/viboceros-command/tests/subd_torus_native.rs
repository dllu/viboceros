#[path = "support/subd_torus_surface.rs"]
mod torus_surface;
use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_torus.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn torus_modes_sizes_and_frames_match_native_geometry_sources_and_history() {
    let registry = CommandRegistry::with_builtins();
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["cancel"] != true && !native::getter_only(r))
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
        assert_eq!(scene.document.undo_label(), Some("SubDTorus"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_TORUS_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_torus_scripts_preserve_model_redo_and_all_preferences() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 99,99,99").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    let prefs = registry.subd_torus_preferences();
    for args in [
        "0,0,0 3 0",
        "0,0,0 3 -1",
        "0,0,0 3 3",
        "0,0,0 3 4",
        "0,0,0 3 Diameter 6",
        "0,0,0 0 1",
        "AroundFaces=2 0,0,0 3 1",
        "VerticalFaces=2 0,0,0 3 1",
        "AroundFaces=100000 VerticalFaces=100000 0,0,0 3 1",
        "0,0,0 3 AroundFaces=8 1",
        "FixInnerDimension=Yes 0,0,0 3 1",
        "0,0,0 3 Area 1",
        "0,0,0 3 FixInnerDimension=Maybe 1",
        "0,0,0 3 1 trailing",
        "0,0,0 3",
        "0,0,0 3 nan",
    ] {
        let cmd = format!("SubDTorus {args}");
        assert!(registry.execute(&mut doc, &cmd).is_err(), "{cmd}");
        assert_eq!(format!("{doc:?}"), before, "{cmd}");
        assert_eq!(registry.subd_torus_preferences(), prefs);
        assert_eq!(registry.subd_cylinder_preferences(), Default::default());
        assert_eq!(registry.subd_sphere_preferences(), Default::default());
        assert_eq!(
            registry.subd_truncated_cone_preferences(),
            viboceros_command::subd_axial::Kind::TruncatedCone.defaults()
        );
    }
}
