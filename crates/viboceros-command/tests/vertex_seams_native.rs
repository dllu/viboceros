#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use serde_json::json;
#[path = "support/vertex_seams.rs"]
mod fixtures;
use viboceros_command::vertex_seams::VertexSeamEdit;

#[test]
fn vertex_seams_match_native_tags_facets_grips_metadata_order_and_history() {
    let rows = fixtures::captures();
    assert_eq!(rows.len(), 46);
    let mut failures = Vec::new();
    for (family, row) in rows {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let v = &row["value"];
            assert_eq!(v["success"], true);
            let (mut doc, ids) = native::setup_from_family(&row, family);
            let mut maximum = native::check(&doc, &v["before"], true);
            let original = ids
                .iter()
                .map(|&id| doc.object(id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            let targets = fixtures::targets(&ids, v);
            let mode = fixtures::mode(v);
            let outcome = VertexSeamEdit::prepare(&doc, mode, targets)
                .unwrap()
                .commit(&mut doc)
                .unwrap();
            for (&id, attributes) in ids.iter().zip(&original) {
                assert_eq!(doc.object(id).unwrap().attributes(), attributes);
            }
            maximum = maximum.max(native::check(&doc, &v["after"], true));
            let changed = v["undo"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["name"] == "Undo witness");
            assert_eq!(!outcome.processed_objects.is_empty(), changed);
            assert_eq!(
                doc.undo_label(),
                Some(if changed {
                    mode.command()
                } else {
                    "Undo witness"
                })
            );
            if v["input"]["grips"] == true {
                doc.disable_control_points();
            }
            doc.clear_selection();
            maximum = maximum.max(native::check(&doc, &v["after_script"], true));
            doc.undo().unwrap();
            maximum = maximum.max(native::check(&doc, &v["undo"], true));
            doc.redo().unwrap();
            maximum = maximum.max(native::check(&doc, &v["redo"], true));
            println!(
                "VERTEX_SEAM_COMMAND {}",
                json!({"id":row["id"],"maximum_error":maximum,"identity":true,"attributes":true,"history":true,"processed_objects":outcome.processed_objects.len()})
            );
        }));
        if result.is_err() {
            failures.push(row["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(failures.is_empty(), "vertex seam differences: {failures:?}");
}
