use super::*;

#[test]
fn permanent_edge_split_fixture_matches_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_split_edge.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_split_edge.json"),
        27,
    );
}

#[test]
fn permanent_edge_collapse_fixture_matches_recorded_rhino_geometry_and_order() {
    assert_recorded_geometry(
        include_str!("../../../tools/rhino_oracle/fixtures/mesh_collapse_edge.json"),
        include_str!("../../../tools/rhino_oracle/observations/mesh_collapse_edge.json"),
        15,
    );
}

fn assert_recorded_geometry(request: &str, recorded: &str, expected_count: usize) {
    let request: ProbeRequest = serde_json::from_str(request).unwrap();
    let recorded: ProbeResponse = serde_json::from_str(recorded).unwrap();
    assert_eq!(recorded.engine, "rhino");
    assert_eq!(recorded.error, None);
    assert_eq!(recorded.protocol_version, request.protocol_version);
    assert_eq!(recorded.iterations, request.iterations);
    let native = run_request(&request).unwrap();
    assert_eq!(native.results.len(), expected_count);
    assert_eq!(native.results.len(), recorded.results.len());
    for (native, recorded) in native.results.iter().zip(&recorded.results) {
        assert_eq!(native.id, recorded.id);
        assert_eq!(native.value, recorded.value, "{}", native.id);
    }
}
