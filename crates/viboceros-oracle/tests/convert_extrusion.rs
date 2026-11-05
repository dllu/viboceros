#[path = "support/edge_analysis_mixed.rs"]
mod comparison;
#[path = "support/observation.rs"]
mod observation;

#[test]
fn native_extrusion_conversion_pilot_identity_attributes_geometry_and_history() {
    replay("convert_extrusion_pilot", 5);
}
#[test]
fn native_extrusion_conversion_all_shared_sources_groups_and_order() {
    replay("convert_extrusion", 60);
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
        for phase in ["before", "after_script", "undo", "redo"] {
            comparison::compare(
                &a["value"][phase],
                &b["value"][phase],
                &format!("{}/{phase}", a["id"]),
            );
        }
    }
}
