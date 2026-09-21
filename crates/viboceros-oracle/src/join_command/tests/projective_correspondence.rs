use super::*;

#[test]
fn projective_discovery_keeps_matches_uncertainty_integral_and_topology_limits_distinct() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_projective_correspondence.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_projective_correspondence.json"
    ))
    .unwrap();
    let native = run_request_audit(&request).unwrap();
    assert_eq!(native.outcomes.len(), 60);
    assert_eq!(expected["results"].as_array().unwrap().len(), 60);
    let (mut matched, mut areas, mut uncertainty, mut nonprojective) = (0, 0, 0, 0);
    for (outcome, reference) in native
        .outcomes
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let id = reference["id"].as_str().unwrap();
        let expected = &reference["value"];
        let result = match outcome {
            OperationOutcome::Failure { id, error } => panic!("{id}: {error:?}"),
            OperationOutcome::Success { result } => result,
        };
        assert_eq!(result.id, id);
        if id.starts_with("nonprojective-square-speed-") {
            // B(s)=A(s^2) has exactly the same curved locus. Its vanishing
            // endpoint speed cannot be represented by our positive projective
            // candidates. Preserve this limitation rather than sampling a join.
            let outputs = surfaces::outputs(expected);
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0]["brep"]["edges"].as_array().unwrap().len(), 7);
            assert_eq!(expected["succeeded"], true);
            assert_eq!(result.value["succeeded"], id.ends_with("-pre"));
            assert_eq!(
                surfaces::outputs(&result.value).len(),
                if id.ends_with("-pre") { 2 } else { 0 }
            );
            for object in result.value["objects"].as_array().unwrap() {
                assert_eq!(object["brep"]["faces"].as_array().unwrap().len(), 1);
                assert!(
                    object["brep"]["edges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|e| e["uses"] == 1)
                );
            }
            nonprojective += 1;
        } else if id.starts_with("speed-16-") || id.starts_with("multispan-speed-") {
            verify_area_only(&result.value, expected, id);
            areas += 1;
        } else if id.starts_with("interior-near-speed-") {
            let mut compared = result.value.clone();
            let mut fields = 0;
            for (object, reference) in compared["objects"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(expected["objects"].as_array().unwrap())
            {
                if !object["source"].is_null() {
                    continue;
                }
                assert_eq!(object["brep"]["edge_tolerances"][0], json!(0.00025));
                assert_eq!(
                    reference["brep"]["edge_tolerances"][0],
                    json!(0.00024793388429752067)
                );
                object["brep"]["edge_tolerances"][0] =
                    reference["brep"]["edge_tolerances"][0].clone();
                fields += 1;
            }
            assert_eq!(fields, 1);
            compare(&compared, expected, id);
            uncertainty += 1;
        } else {
            compare(&result.value, expected, id);
            matched += 1;
        }
    }
    assert_eq!((matched, areas, uncertainty, nonprojective), (44, 8, 4, 4));
}

fn verify_area_only(actual: &Value, expected: &Value, id: &str) {
    // Extruding (30t,30t(1-t),0) by length 3 has this analytic area,
    // independent of its parameter speed or inserted knots. The reference
    // value was also checked at 80 decimal digits, without either kernel.
    let analytic = 45. * (2_f64.sqrt() + 1_f64.asinh());
    let mut compared = actual.clone();
    let mut fields = 0;
    for (object, reference) in compared["objects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(expected["objects"].as_array().unwrap())
    {
        for (face, reference) in object["brep"]["faces"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(reference["brep"]["faces"].as_array().unwrap())
        {
            let (n, r) = (face[1].as_f64().unwrap(), reference[1].as_f64().unwrap());
            if (n - r).abs() <= 1e-10_f64.max(1e-12 * n.abs().max(r.abs())) {
                continue;
            }
            assert!((n - analytic).abs() < 2e-13);
            let rhino = if id.starts_with("speed-16-") {
                103.30142172278235
            } else {
                103.30142172316334
            };
            assert_eq!(r, rhino);
            face[1] = reference[1].clone();
            fields += 1;
        }
    }
    assert_eq!(fields, if id.contains("JoinCopy") { 2 } else { 1 });
    // These remain discrepancies; only after checking them explicitly do we
    // verify that every other raw geometry and document field agrees.
    compare(&compared, expected, id);
}
