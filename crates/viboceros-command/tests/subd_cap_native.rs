//! Native public-command replays from independently authored input archives.
#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
#[path = "support/subd_cap.rs"]
mod native;
#[allow(dead_code)]
#[path = "support/crease.rs"]
mod scene;
#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;
use viboceros_command::CommandRegistry;

#[test]
fn subd_cap_replays_native_options_metadata_and_history() {
    let mut diagnostics = Vec::new();
    for (family, row) in native::rows() {
        let value = &row["value"];
        let (mut doc, ids) = native::setup(&row, family);
        let original = ids
            .iter()
            .map(|&id| doc.object(id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        let before = native::compare(&doc, &ids, &value["before"]);
        CommandRegistry::with_builtins()
            .execute(&mut doc, &native::command(value))
            .unwrap_or_else(|e| panic!("{} {e}", row["id"]));
        let after = native::compare(&doc, &ids, &value["after"]);
        doc.clear_selection();
        let after_script = native::compare(&doc, &ids, &value["after_script"]);
        doc.undo().unwrap();
        for (&id, snapshot) in ids.iter().zip(&original) {
            assert!(
                doc.object(id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(snapshot)
            );
        }
        let undo = native::compare(&doc, &ids, &value["undo"]);
        doc.redo().unwrap();
        let redo = native::compare(&doc, &ids, &value["redo"]);
        let report = serde_json::json!({"id":row["id"],"before":before,"after":after,"after_script":after_script,"undo":undo,"redo":redo});
        println!("SUBD_CAP {report}");
        diagnostics.push(report);
    }
    native::assert_diagnostics(&diagnostics);
}
