#[path = "support/rebuild_normals.rs"]
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
fn native_rebuild_normals_commands_match_geometry_selection_order_and_history() {
    let registry = CommandRegistry::with_builtins();
    for (family, rows) in fixtures::groups(true) {
        for row in rows {
            let value = &row["value"];
            if value["success"] != true {
                continue;
            }
            eprintln!("Rebuild normals command {}", row["id"]);
            let (mut doc, ids) = native::setup_from_family(&row, family);
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            if value["input"]["preselected"] == false {
                registry
                    .execute_postselected(&mut doc, "RebuildMeshNormals", Default::default())
                    .unwrap();
            } else {
                registry.execute(&mut doc, "RebuildMeshNormals").unwrap();
            }
            let mut maximum = native::mesh_command::check(&doc, &ids, value, "after");
            assert_eq!(doc.undo_label(), Some("RebuildMeshNormals"));
            doc.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, value, "undo"));
            doc.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, value, "redo"));
            println!(
                "REBUILD_NORMALS_COMMAND {}",
                fixtures::report(&row, maximum)
            );
        }
    }
}
