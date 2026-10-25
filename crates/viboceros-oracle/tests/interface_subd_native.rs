use viboceros_oracle::{ProbeRequest, run_request};

#[test]
fn interface_subd_states_match_native_transitions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let request: ProbeRequest = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/interface_subd.json")).unwrap(),
    )
    .unwrap();
    let native: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/interface_subd.json")).unwrap(),
    )
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 64);
    for (row, expected) in actual
        .results
        .iter()
        .zip(native["results"].as_array().unwrap())
    {
        assert_eq!(row.id, expected["id"]);
        assert_eq!(row.value, expected["value"], "{}", row.id);
        println!(
            "INTERFACE_SUBD_ORACLE {}",
            serde_json::json!({"id":row.id,"value":row.value})
        );
    }
}
