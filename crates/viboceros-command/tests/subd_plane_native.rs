use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
#[allow(dead_code)]
mod box_native;
#[path = "support/subd_plane.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn subd_planes_match_native_counts_construction_modes_boundary_tags_and_history() {
    for row in native::rows().into_iter().filter(|r| {
        r["value"]["input"]["unsupported"] != true && r["value"]["input"]["cancel"] != true
    }) {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute_in_context(
                &mut scene.document,
                &native::command(value, scene.source),
                CommandContext {
                    construction_plane: native::plane(value),
                },
            )
            .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
        let check = native::check(&scene, value, "after_script");
        assert_eq!(scene.document.undo_label(), Some("SubDPlane"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_PLANE_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_planes_and_counts_preserve_redo_and_command_preferences() {
    let mut doc = viboceros_document::Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for command in [
        "SubDPlane XCount=0 0,0,0 4,3,0",
        "SubDPlane XCount=100000 YCount=100000 0,0,0 4,3,0",
        "SubDPlane ZCount=2 0,0,0 4,3,0",
        "SubDPlane 0,0,0 4,0,0",
        "SubDPlane EdgeMidpoint 0,0,0 4,3,0 1,2,3",
        "SubDPlane 0,0,0 4,3,0 extra",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before);
    }
    assert_eq!(registry.subd_plane_preferences(), Default::default());
}
