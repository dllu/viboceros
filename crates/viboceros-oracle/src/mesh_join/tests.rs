use super::*;

fn request() -> ProbeRequest {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/mesh_join.json"
    ))
    .unwrap()
}

#[test]
fn every_mesh_join_field_replays_raw_rhino_without_normalization() {
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/mesh_join.json"
    ))
    .unwrap();
    let response = run_request(&request()).unwrap();
    assert_eq!(response.results.len(), 181);
    let records = expected["results"].as_array().unwrap();
    assert_eq!(records.len(), 181);
    for (actual, expected) in response.results.iter().zip(records) {
        assert_eq!(actual.id, expected["id"]);
        assert_eq!(actual.value, expected["value"], "{}", actual.id);
    }
}

#[test]
fn malformed_mesh_join_fixtures_are_rejected_before_execution() {
    let Operation::MeshJoinCommand { fixture, .. } = request().operations.remove(0) else {
        panic!("mesh join")
    };
    for indices in [vec![], vec![0, 0], vec![2]] {
        let mut invalid = fixture.clone();
        invalid.selected = Some(indices);
        assert!(run(&invalid, Tolerance::DEFAULT).is_err());
    }
    for tolerance in [f64::NAN, f64::INFINITY, 0., -1.] {
        let mut invalid = fixture.clone();
        invalid.absolute_tolerance = Some(tolerance);
        assert!(run(&invalid, Tolerance::DEFAULT).is_err());
    }
    let mut invalid = fixture.clone();
    invalid.sources.clear();
    assert!(run(&invalid, Tolerance::DEFAULT).is_err());
    let mut invalid = fixture.clone();
    invalid.sources = vec![fixture.sources[0].clone(); 33];
    assert!(run(&invalid, Tolerance::DEFAULT).is_err());
}
