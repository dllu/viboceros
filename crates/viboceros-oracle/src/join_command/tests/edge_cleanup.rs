use super::*;

#[test]
fn redundant_seams_coalesce_at_small_angles_without_copying_cosine_roundoff() {
    for (fixture, observation) in [
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/join_edge_cleanup_tight.json"),
            include_str!(
                "../../../../../tools/rhino_oracle/observations/join_edge_cleanup_tight.json"
            ),
        ),
        (
            include_str!(
                "../../../../../tools/rhino_oracle/fixtures/join_edge_cleanup_roundoff.json"
            ),
            include_str!(
                "../../../../../tools/rhino_oracle/observations/join_edge_cleanup_roundoff.json"
            ),
        ),
        (
            include_str!(
                "../../../../../tools/rhino_oracle/fixtures/join_edge_cleanup_resolved.json"
            ),
            include_str!(
                "../../../../../tools/rhino_oracle/observations/join_edge_cleanup_resolved.json"
            ),
        ),
        (
            include_str!(
                "../../../../../tools/rhino_oracle/fixtures/join_edge_cleanup_degree.json"
            ),
            include_str!(
                "../../../../../tools/rhino_oracle/observations/join_edge_cleanup_degree.json"
            ),
        ),
    ] {
        let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
        let reference: Value = serde_json::from_str(observation).unwrap();
        let native = run_request_audit(&request).unwrap();
        assert_eq!(native.outcomes.len(), 8);
        for (outcome, reference) in native
            .outcomes
            .iter()
            .zip(reference["results"].as_array().unwrap())
        {
            let result = match outcome {
                OperationOutcome::Success { result } => result,
                OperationOutcome::Failure { id, error } => panic!("{id}: {error:?}"),
            };
            let id = &result.id;
            assert_eq!(id, reference["id"].as_str().unwrap());
            let objects = surfaces::outputs(&result.value);
            assert_eq!(objects.len(), 1);
            let brep = &objects[0]["brep"];
            assert_eq!(brep["edges"].as_array().unwrap().len(), 7, "{id}");
            assert_eq!(brep["vertices"].as_array().unwrap().len(), 6, "{id}");
            let mates = brep["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["uses"] == 2)
                .collect::<Vec<_>>();
            assert_eq!(mates.len(), 1, "{id}");
            for p in mates[0]["curve"]["definition"]["control_points"]
                .as_array()
                .unwrap()
            {
                assert!(
                    (p["point"][2].as_f64().unwrap() - 0.00025).abs() < 1e-18,
                    "{id}"
                );
            }
            let expected = &reference["value"];
            let mut state = result.value.clone();
            let mut expected_state = expected.clone();
            state.as_object_mut().unwrap().remove("objects");
            expected_state.as_object_mut().unwrap().remove("objects");
            compare(&state, &expected_state, id);
            // The observed tiny-angle topology varies by representation. Keep
            // it raw and visible; only the resolved-angle cases share counts.
            if request.tolerance.angular >= 1e-6 {
                let expected_objects = surfaces::outputs(expected);
                assert_eq!(
                    expected_objects[0]["brep"]["edges"]
                        .as_array()
                        .unwrap()
                        .len(),
                    7
                );
            }
            if !id.starts_with("unclamped-")
                && (request.tolerance.angular >= 1e-6 || id.starts_with("quadratic-"))
            {
                compare(&result.value, expected, id);
            }
        }
    }
}
