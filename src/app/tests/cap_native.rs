use super::*;
use crate::app::{
    closed_polyline_native_tests as surface, crease_native_tests::fixture as scene,
    subdivision_native_tests as mesh_native,
};
#[path = "../../../crates/viboceros-command/tests/support/cap_regions.rs"]
mod native;

#[test]
fn mesh_cap_preselection_options_and_history_replay_native_sources() {
    let mut diagnostics = Vec::new();
    for (family, row) in native::rows() {
        let value = &row["value"];
        let (doc, ids) = native::setup(&row, family);
        let mut app = test_app();
        app.document = doc;
        let original = ids
            .iter()
            .map(|&id| app.document.object(id).unwrap().geometry_snapshot().clone())
            .collect::<Vec<_>>();
        let before = native::compare(&app.document, &ids, &value["before"], true);
        app.command_input = native::command(value);
        app.run_command();
        if app.object_prompt.is_some() {
            app.command_input.clear();
            app.run_command();
        }
        assert!(
            app.object_prompt.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        let after = native::compare(&app.document, &ids, &value["after"], true);
        app.document.clear_selection();
        let after_script = native::compare(&app.document, &ids, &value["after_script"], true);
        app.document.undo().unwrap();
        for (&id, snapshot) in ids.iter().zip(&original) {
            assert!(
                app.document
                    .object(id)
                    .unwrap()
                    .geometry_snapshot()
                    .shares_storage_with(snapshot)
            );
        }
        let undo = native::compare(&app.document, &ids, &value["undo"], true);
        app.document.redo().unwrap();
        let redo = native::compare(&app.document, &ids, &value["redo"], true);
        let report = serde_json::json!({"id":row["id"],"before":before,"after":after,"after_script":after_script,"undo":undo,"redo":redo});
        println!("APP_CAP_REGIONS {}", report);
        diagnostics.push(report);
    }
    native::assert_diagnostics(&diagnostics);
}
