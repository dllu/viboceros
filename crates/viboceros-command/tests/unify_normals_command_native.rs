#[path = "support/unify_normals.rs"]
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
fn native_unification_commands_match_channels_and_document_lifecycle() {
    for (family, rows) in fixtures::groups() {
        let registry = CommandRegistry::with_builtins();
        for row in rows {
            let v = &row["value"];
            if v["success"] != true {
                continue;
            }
            eprintln!("Mesh unify command {}", row["id"]);
            let (mut doc, ids) = native::setup_from_family(&row, family);
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            let command = "UnifyMeshNormals";
            if v["input"]["preselected"] == false {
                registry
                    .execute_postselected(&mut doc, command, Default::default())
                    .unwrap();
            } else {
                registry.execute(&mut doc, command).unwrap();
            }
            let mut maximum = native::mesh_command::check(&doc, &ids, v, "after");
            doc.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, v, "undo"));
            doc.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, v, "redo"));
            println!("UNIFY_NORMALS_COMMAND {}", fixtures::report(&row, maximum));
        }
    }
}
