//! Independent source archives and complete Cap scene/history comparisons.
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[path = "support/cap_regions.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod scene;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
#[allow(dead_code)]
#[path = "support/closed_polyline.rs"]
mod surface;
use serde_json::json;
use viboceros_command::CommandRegistry;

#[test]
fn cap_replays_exact_sources_options_metadata_and_document_history() {
    let mut diagnostics = Vec::new();
    for (family, row) in native::rows() {
        let value = &row["value"];
        let (mut doc, ids) = native::setup(&row, family);
        let original = ids
            .iter()
            .map(|&id| doc.object(id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        let before = native::compare(&doc, &ids, &value["before"], true);
        CommandRegistry::with_builtins()
            .execute(&mut doc, &native::command(value))
            .unwrap();
        let after = native::compare(&doc, &ids, &value["after"], true);
        doc.clear_selection();
        let after_script = native::compare(&doc, &ids, &value["after_script"], true);
        doc.undo().unwrap();
        for (&id, snapshot) in ids.iter().zip(&original) {
            assert!(
                doc.object(id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(snapshot)
            );
        }
        let undo = native::compare(&doc, &ids, &value["undo"], true);
        doc.redo().unwrap();
        let redo = native::compare(&doc, &ids, &value["redo"], true);
        let report = json!({"id":row["id"],"before":before,"after":after,"after_script":after_script,"undo":undo,"redo":redo});
        println!("CAP_REGIONS {}", report);
        diagnostics.push(report);
    }
    native::assert_diagnostics(&diagnostics);
}
