use super::*;

#[test]
fn corner_discovery_audits_every_case_even_after_transitive_cluster_rejections() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_corner_clustering.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_corner_clustering.json"
    ))
    .unwrap();
    assert!(run_request(&request).is_err());
    let native = run_request_audit(&request).unwrap();
    assert_eq!(native.outcomes.len(), 84);
    assert_eq!(expected["results"].as_array().unwrap().len(), 84);
    let (mut matched, mut failed, mut differing) = (0, 0, 0);
    for (outcome, reference) in native
        .outcomes
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let id = reference["id"].as_str().unwrap();
        let value = &reference["value"];
        // No full edge is mated in any of these corner-only commands.
        for object in value["objects"].as_array().unwrap() {
            assert_eq!(object["brep"]["faces"].as_array().unwrap().len(), 1);
            assert_eq!(object["brep"]["edges"].as_array().unwrap().len(), 4);
            assert!(
                object["brep"]["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["uses"] == 1)
            );
        }
        let result = match outcome {
            OperationOutcome::Failure { id: actual, error } => {
                assert_eq!(actual, id);
                assert!(id.starts_with("corners-chain4-") || id.starts_with("corners-chain5-"));
                assert_eq!(error.kind, "command");
                assert!(
                    error
                        .message
                        .contains("adjusted boundary cluster exceeds the join distance")
                );
                failed += 1;
                continue;
            }
            OperationOutcome::Success { result } => result,
        };
        assert_eq!(result.id, id);
        let matches = id.starts_with("corner-pair-")
            || id.starts_with("corners-close-")
            || id.starts_with("asym-close-")
            || [
                "corners-chain3-102",
                "corners-chain3-120",
                "asym-binary-102",
                "asym-binary-120",
            ]
            .contains(&id);
        if matches {
            compare(&result.value, value, id);
            matched += 1;
        } else {
            // These remain geometry discrepancies, not normalized matches.
            // Still verify every output's unchanged underlying surface and
            // document fields, and require a visible corner disagreement.
            let (n, r) = (surfaces::outputs(&result.value), surfaces::outputs(value));
            assert_eq!(n.len(), r.len());
            let mut different = false;
            for (n, r) in n.into_iter().zip(r) {
                for field in ["source", "name", "layer", "color", "groups", "selected"] {
                    compare(&n[field], &r[field], id);
                }
                compare(&n["brep"]["surfaces"], &r["brep"]["surfaces"], id);
                let corner = |o: &Value| {
                    o["brep"]["vertices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|p| p[0].as_f64() == Some(0.) && p[1].as_f64() == Some(0.))
                        .unwrap()[2]
                        .as_f64()
                        .unwrap()
                };
                different |= (corner(n) - corner(r)).abs() > 1e-10;
            }
            assert!(different, "{id}");
            differing += 1;
        }
    }
    assert_eq!((matched, failed, differing), (44, 10, 30));
}
