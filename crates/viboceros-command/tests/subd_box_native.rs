use viboceros_command::{CommandContext, CommandRegistry};
#[path = "support/subd_box.rs"]
mod native;
#[path = "support/subd.rs"]
#[allow(dead_code)]
mod subd_native;
#[test]
fn subd_boxes_match_native_modes_counts_limit_surfaces_and_history() {
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
        assert_eq!(scene.document.undo_label(), Some("SubDBox"));
        scene.document.undo().unwrap();
        native::check(&scene, value, "undo");
        scene.document.redo().unwrap();
        assert_eq!(native::check(&scene, value, "redo"), check);
        println!(
            "SUBD_BOX_COMMAND {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn invalid_counts_modes_and_degenerate_boxes_keep_geometry_selection_and_redo() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = viboceros_document::Document::default();
    registry.execute(&mut doc, "Point 9,9,9").unwrap();
    doc.undo().unwrap();
    let before = format!("{doc:?}");
    for command in [
        "SubDBox XCount=0 0,0,0 4,3,0 5",
        "SubDBox XCount=9999999999999999999 0,0,0 4,3,0 5",
        "SubDBox XCount=100000 YCount=100000 ZCount=100000 0,0,0 4,3,0 5",
        "SubDBox 0,0,0 4,3,0 0",
        "SubDBox 0,0,0 4,0,0 5",
        "SubDBox Cube 0,0,0 4,3,5",
        "SubDBox Diagonal 0,0,0 4,3,5",
        "SubDBox EdgeMidpoint 0,0,0 4,3,0 5",
        "SubDBox 0,0,0 4,3,0 5 extra",
    ] {
        assert!(registry.execute(&mut doc, command).is_err(), "{command}");
        assert_eq!(format!("{doc:?}"), before, "{command}");
    }
    assert_eq!(registry.subd_box_preferences(), Default::default());
}

#[test]
fn three_point_option_after_first_corner_keeps_the_original_anchor() {
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "subd_box_three_point")
        .unwrap();
    let value = &row["value"];
    let mut scene = native::setup(value);
    let mut actions = native::actions(value, scene.source);
    actions.swap(0, 1);
    let command = format!("{} {}", native::header(value), actions.join(" "));
    CommandRegistry::with_builtins()
        .execute_in_context(
            &mut scene.document,
            &command,
            CommandContext {
                construction_plane: native::plane(value),
            },
        )
        .unwrap();
    native::check(&scene, value, "after_script");
    assert_eq!(scene.document.undo_label(), Some("SubDBox"));
}
