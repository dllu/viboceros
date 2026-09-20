use super::*;

#[test]
fn curve_alignment_matches_all_saved_rhino_shapes_targets_and_selection() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/align_curve.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/align_curve.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 39);
    assert_eq!(reference["results"].as_array().unwrap().len(), 39);
    for (a, b) in actual
        .results
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn malformed_curve_targets_do_not_become_implicit_points_or_default_selection() {
    let valid: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/align_curve.json"
    ))
    .unwrap();
    for (field, value) in [
        ("curve", json!(null)),
        ("curve", json!(true)),
        ("curve", json!(99)),
        ("curve", json!(0)),
        ("selected", json!([0, 1, 2, 3])),
        ("mode", json!("ToFitPlane")),
        ("target", json!([0, 0, 0])),
        ("references", json!([[0, 0, 0]])),
    ] {
        let mut request = valid.clone();
        request["operations"].as_array_mut().unwrap().truncate(1);
        request["operations"][0][field] = value;
        if let Ok(request) = serde_json::from_value::<ProbeRequest>(request) {
            assert!(run_request(&request).is_err(), "{field}");
        }
    }
}
