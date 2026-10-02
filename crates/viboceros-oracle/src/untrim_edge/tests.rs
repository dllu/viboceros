use super::*;

#[test]
fn independent_untrim_sources_match_native_geometry_metadata_and_history() {
    for (input, capture, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_components.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_components.json"),
            71,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_partial.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_partial.json"),
            6,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_upper.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_upper.json"),
            4,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_history.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_history.json"),
            24,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_ordering.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_ordering.json"),
            100,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_curved_partial.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_curved_partial.json"),
            8,
        ),
    ] {
        let request: ProbeRequest = serde_json::from_str(input).unwrap();
        let native: Value = serde_json::from_str(capture).unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), count);
        let mut failures = Vec::new();
        for (actual, expected) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(actual.id, expected["id"]);
            let mut expected = expected["value"].clone();
            for diagnostic in ["events", "history", "undo_events", "redo_events"] {
                expected.as_object_mut().unwrap().remove(diagnostic);
            }
            // Numeric component indices are observable command behavior. Compare
            // the complete native tables without renumbering any edge or trim.
            if std::panic::catch_unwind(|| {
                crate::test_json::close(&actual.value, &expected, &actual.id, 1e-9, 0.)
            })
            .is_err()
            {
                failures.push(actual.id.clone());
            }
        }
        assert!(failures.is_empty(), "unmatched native cases: {failures:?}");
    }
}

#[test]
fn invalid_indices_and_ambiguous_history_iterations_are_rejected() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/untrim_components.json"
    ))
    .unwrap();
    request["operations"] = json!([request["operations"][0].clone()]);
    for (field, value) in [
        ("sources", json!([])),
        ("components", json!([[1, 0]])),
        ("components", json!([[0, 100]])),
        ("components", json!(vec![[0, 0]; 65])),
        ("view", json!("oblique")),
    ] {
        let mut invalid = request.clone();
        invalid["operations"][0][field] = value;
        let request: ProbeRequest = serde_json::from_value(invalid).unwrap();
        assert!(run_request(&request).is_err());
    }
    request["iterations"] = json!(2);
    let request: ProbeRequest = serde_json::from_value(request).unwrap();
    assert!(run_request(&request).is_err());
}
