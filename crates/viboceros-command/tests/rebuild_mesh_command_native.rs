#[path = "support/rebuild_mesh.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use viboceros_command::CommandRegistry;
use viboceros_document::SelectionMode;

#[test]
fn native_mesh_rebuild_commands_match_channels_preferences_and_document_lifecycle() {
    for (family, rows) in fixtures::groups() {
        let registry = CommandRegistry::with_builtins();
        for row in rows {
            let v = &row["value"];
            if v["success"] != true {
                continue;
            }
            eprintln!("Mesh rebuild command {}", row["id"]);
            let before = registry.rebuild_mesh_options(&[]).unwrap();
            let expected = fixtures::staged(v, before);
            let (mut doc, ids) = native::setup_from_family(&row, family);
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            let command = format!("RebuildMesh {}", fixtures::arguments(v).join(" "));
            if v["input"]["preselected"] == false {
                registry
                    .execute_postselected(&mut doc, &command, Default::default())
                    .unwrap();
            } else {
                registry.execute(&mut doc, &command).unwrap();
            }
            let actual = registry.rebuild_mesh_options(&[]).unwrap();
            assert_eq!(actual, expected);
            let mut maximum = native::mesh_command::check(&doc, &ids, v, "after");
            assert_eq!(doc.undo_label(), Some("RebuildMesh"));
            doc.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, v, "undo"));
            assert_eq!(registry.rebuild_mesh_options(&[]).unwrap(), actual);
            doc.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, v, "redo"));
            assert_eq!(registry.rebuild_mesh_options(&[]).unwrap(), actual);
            println!(
                "REBUILD_MESH_COMMAND {}",
                fixtures::report(&row, actual, maximum)
            );
        }
    }
}
