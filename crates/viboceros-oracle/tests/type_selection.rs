use serde_json::Value;
#[path = "support/edge_analysis_mixed.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;

#[test]
fn native_block_and_extrusion_selection_names_groups_visibility_and_redo() {
    replay("type_selection_clean", 35);
    replay("type_selection_empty", 8);
}
fn replay(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let actual = serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
    let native = observation::read(&base, family);
    let actual = actual["results"].as_array().unwrap();
    let native = native["results"].as_array().unwrap();
    assert_eq!(actual.len(), count);
    assert_eq!(actual.len(), native.len());
    for (a, b) in actual.iter().zip(native) {
        assert_eq!(a["id"], b["id"]);
        for phase in ["before", "after", "repeat", "previous", "redo"] {
            let actual: &Value = &a["value"][phase];
            comparison::compare(actual, &b["value"][phase], &format!("{}/{phase}", a["id"]));
            assert_eq!(actual["geometry_unchanged"], true);
        }
    }
}
