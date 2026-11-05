use serde_json::Value;
#[path = "support/edge_analysis_mixed.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;

#[test]
fn native_edge_analysis_deletion_undo_redo_and_explicit_removal() {
    replay("edge_analysis_lifetime", 13);
}
#[test]
fn native_edge_analysis_runtime_versions_and_geometry_edit_replay() {
    replay("edge_analysis_lifetime_runtime", 29);
}
#[test]
fn native_edge_analysis_geometry_edits_with_physical_panel_input() {
    replay("edge_analysis_lifetime_ui", 29);
}
#[test]
fn native_edge_analysis_panel_selection_cleanup() {
    replay("edge_analysis_panel_selection", 14);
    replay("edge_analysis_modes", 11);
}
fn replay(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, family);
    let actual = serde_json::to_value(
        viboceros_oracle::run_request(&serde_json::from_value(request).unwrap()).unwrap(),
    )
    .unwrap();
    let rows = actual["results"].as_array().unwrap();
    let references = native["results"].as_array().unwrap();
    assert_eq!(rows.len(), count);
    assert_eq!(rows.len(), references.len());
    for (a, b) in rows.iter().zip(references) {
        assert_eq!(a["id"], b["id"]);
        for phase in ["before", "after"] {
            comparison::compare(
                &a["value"][phase],
                &b["value"][phase],
                &format!("{}/{phase}", a["id"]),
            );
        }
        let stages = a["value"]["stages"].as_array().unwrap();
        let native_stages = b["value"]["stages"].as_array().unwrap();
        assert_eq!(stages.len(), native_stages.len(), "{}", a["id"]);
        for (s, t) in stages.iter().zip(native_stages) {
            assert_eq!(s["action"], t["action"]);
            if t.get("selection_states").is_some() {
                assert_eq!(
                    s["selection_states"], t["selection_states"],
                    "{} {}",
                    a["id"], s["action"]
                );
            }
            comparison::compare(
                &s["state"],
                &t["state"],
                &format!("{}/{}", a["id"], s["action"]),
            );
        }
    }
}
