#[path = "support/apex_splitting/cap_chart.rs"]
mod cap_charts;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/extrude_apex_subd.rs"]
mod subd_apex_comparison;
#[path = "support/extrude_curve_subd.rs"]
mod subd_face_comparison;
#[path = "support/revolve_subd.rs"]
mod subd_profile_comparison;
#[path = "support/extrusion_splitting.rs"]
mod topology;
#[test]
fn straight_extrusion_split_policies_match_native_definitions_and_history() {
    family("extrusion_splitting_pilot", 55);
    family("extrusion_splitting_holdouts", 58);
    family("extrusion_splitting_confirmations", 10);
    family("extrusion_splitting_cancellations", 2);
    family("extrusion_splitting_curved", 22);
    family("extrusion_splitting_mixed", 16);
}

fn family(family: &str, count: usize) {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let query: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let native = observation::read(&base, family);
    assert_eq!(query["operations"].as_array().unwrap().len(), count);
    for operation in query["operations"].as_array().unwrap() {
        let mut one = query.clone();
        one["operations"] = serde_json::json!([operation]);
        let request = serde_json::from_value(one).unwrap();
        let response = viboceros_oracle::run_request(&request)
            .unwrap_or_else(|e| panic!("{}: {e:?}", operation["id"]));
        let actual = serde_json::to_value(response).unwrap();
        let row = &actual["results"][0];
        let expected = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == row["id"])
            .unwrap();
        for phase in ["before", "after", "undo", "redo"] {
            topology::compare(
                &row["value"][phase],
                &expected["value"][phase],
                &format!("{}/{phase}", row["id"]),
                1e-12,
            );
        }
    }
}
