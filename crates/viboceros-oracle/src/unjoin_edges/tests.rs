use super::*;
#[test]
fn complete_separation_outputs_match_native_on_the_independently_shared_sources() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/brep_unjoin_edges.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/brep_unjoin_edges.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 26);
    for (actual, native) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(actual.id, native["id"]);
        crate::test_json::close(&actual.value, &native["value"], &actual.id, 1e-9, 0.);
    }
}
