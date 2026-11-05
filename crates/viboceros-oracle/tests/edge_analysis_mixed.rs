use serde_json::Value;
#[path = "support/edge_analysis_mixed.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;

#[test]
fn native_mixed_diagnostic_source_order_selection_groups_and_history() {
    replay("edge_analysis_mixed", 72);
    replay("edge_analysis_mixed_ui", 14);
    replay("edge_analysis_mixed_selection", 6);
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
    let a = actual["results"].as_array().unwrap();
    let b = native["results"].as_array().unwrap();
    assert_eq!(a.len(), count);
    assert_eq!(b.len(), count);
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a["id"], b["id"]);
        for phase in ["before", "after", "undo", "redo"] {
            comparison::compare(
                &a["value"][phase],
                &b["value"][phase],
                &format!("{}/{phase}", a["id"]),
            );
        }
        if let Some(native) = b["value"]["stages"].as_array() {
            let stages = a["value"]["stages"].as_array().unwrap();
            assert_eq!(stages.len(), native.len());
            for (stage, b) in stages.iter().zip(native) {
                assert_eq!(stage["action"], b["action"]);
                comparison::compare(
                    &stage["state"],
                    &b["state"],
                    &format!("{}/{}", a["id"], stage["action"]),
                );
            }
        }
    }
}
