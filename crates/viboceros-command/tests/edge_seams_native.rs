#[path = "support/edge_seams.rs"]
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
use serde_json::json;
use viboceros_command::{
    component_seams::{ComponentSeamEdit, SeamTarget},
    crease::CreaseTarget,
};

#[test]
fn edge_seams_match_native_geometry_tags_metadata_order_and_atomic_history() {
    let rows = fixtures::captures()
        .into_iter()
        .filter(|(_, r)| r["value"]["success"] == true)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 66);
    let mut failures = Vec::new();
    for (family, row) in rows {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let v = &row["value"];
            let (mut doc, ids) = native::setup_from_family(&row, family);
            let mut maximum = native::check(&doc, &v["before"], true);
            let attributes = ids
                .iter()
                .map(|&id| doc.object(id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            let targets = native::targets(&doc, &ids, v).into_iter().map(|t| match t {
                CreaseTarget::Edge(object, index) => SeamTarget { object, index },
                _ => panic!("expected edge"),
            });
            let mode = fixtures::mode(v);
            let out = ComponentSeamEdit::prepare(&doc, mode, targets)
                .unwrap()
                .commit(&mut doc)
                .unwrap();
            for (&id, attr) in ids.iter().zip(attributes) {
                assert_eq!(doc.object(id).unwrap().attributes(), &attr);
            }
            maximum = maximum.max(native::check(&doc, &v["after"], true));
            let changed = v["undo"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["name"] == "Undo witness");
            assert_eq!(!out.processed_objects.is_empty(), changed);
            assert_eq!(
                doc.undo_label(),
                Some(if changed {
                    mode.command()
                } else {
                    "Undo witness"
                })
            );
            doc.clear_selection();
            maximum = maximum.max(native::check(&doc, &v["after_script"], true));
            doc.undo().unwrap();
            maximum = maximum.max(native::check(&doc, &v["undo"], true));
            doc.redo().unwrap();
            maximum = maximum.max(native::check(&doc, &v["redo"], true));
            println!(
                "EDGE_SEAM_COMMAND {}",
                json!({"id":row["id"],"maximum_error":maximum,"identity":true,"attributes":true,"history":true,"processed_objects":out.processed_objects.len()})
            );
        }));
        if result.is_err() {
            failures.push(row["id"].as_str().unwrap().to_owned());
        }
    }
    assert!(failures.is_empty(), "edge seam differences: {failures:?}");
}
