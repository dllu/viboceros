#[path = "support/nonplanar_command.rs"]
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
fn native_nonplanar_command_options_geometry_order_and_undo_match() {
    for (family, rows) in fixtures::groups() {
        let registry = CommandRegistry::with_builtins();
        for row in rows {
            let value = &row["value"];
            if value["success"] != true {
                continue;
            }
            eprintln!("Nonplanar command {}", row["id"]);
            let (mut doc, ids) = native::setup_from_family(&row, family);
            let attributes = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
            registry
                .execute(
                    &mut doc,
                    &format!("TriangulateNonPlanarQuads {}", fixtures::arguments(value)),
                )
                .unwrap();
            assert_eq!(doc.undo_label(), Some("TriangulateNonPlanarQuads"));
            for (id, original) in ids.iter().zip(attributes) {
                assert_eq!(doc.object(*id).unwrap().attributes(), &original);
            }
            let mut maximum = native::mesh_command::check(&doc, &ids, value, "after");
            doc.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, value, "undo"));
            doc.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&doc, &ids, value, "redo"));
            println!(
                "NONPLANAR_COMMAND {}",
                fixtures::report(
                    &row,
                    registry.triangulate_nonplanar_quads_options(&[]).unwrap(),
                    maximum
                )
            );
        }
    }
}
