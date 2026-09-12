use super::*;

#[test]
fn permanent_edge_split_fixture_matches_recorded_rhino_geometry_and_order() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_split_edge.json"
    ))
    .unwrap();
    let recorded: ProbeResponse = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_split_edge.json"
    ))
    .unwrap();
    assert_eq!(recorded.engine, "rhino");
    assert_eq!(recorded.error, None);
    assert_eq!(recorded.protocol_version, request.protocol_version);
    assert_eq!(recorded.iterations, request.iterations);
    let native = run_request(&request).unwrap();
    assert_eq!(native.results.len(), 27);
    assert_eq!(native.results.len(), recorded.results.len());
    for (native, recorded) in native.results.iter().zip(&recorded.results) {
        assert_eq!(native.id, recorded.id);
        assert_eq!(native.value, recorded.value, "{}", native.id);
    }
}
