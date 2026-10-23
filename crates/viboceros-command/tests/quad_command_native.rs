#[path = "support/quad_command.rs"]
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
fn native_command_defaults_options_order_selection_and_undo_match() {
    for (family, rows) in fixtures::groups() {
        let registry = CommandRegistry::with_builtins();
        for row in rows {
            let value = &row["value"];
            if value["success"] != true {
                continue;
            }
            eprintln!("Command {}", row["id"]);
            let (mut doc, ids) = native::setup_from_family(&row, family);
            let attributes = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            registry
                .execute(&mut doc, &fixtures::command(value))
                .unwrap();
            let mut maximum = fixtures::check(&doc, &ids, value, "after");
            for (id, original) in ids.iter().zip(&attributes) {
                assert_eq!(doc.object(*id).unwrap().attributes(), original);
            }
            assert_eq!(doc.undo_label(), Some("QuadrangulateMesh"));
            doc.undo().unwrap();
            maximum = maximum.max(fixtures::check(&doc, &ids, value, "undo"));
            doc.redo().unwrap();
            maximum = maximum.max(fixtures::check(&doc, &ids, value, "redo"));
            println!(
                "QUAD_COMMAND {}",
                fixtures::report(
                    &row,
                    registry.quadrangulate_mesh_options(&[]).unwrap(),
                    maximum
                )
            );
        }
    }
}
