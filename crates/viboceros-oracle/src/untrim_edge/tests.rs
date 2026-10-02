use super::*;

/// Assign spatial edge indices by complete face/loop/trim traversal. This is a
/// bijection: no edge geometry or adjacency is omitted. Partial restoration
/// keeps the Rust source table order; Rhino allocates those edge slots differently.
fn edge_traversal(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                edge_traversal(value);
            }
        }
        Value::Object(object) => {
            if object.get("type") == Some(&json!("brep")) {
                let definition = object.get_mut("definition").unwrap();
                let count = definition["edges"].as_array().unwrap().len();
                let mut order = Vec::new();
                for face in definition["topology"]["faces"].as_array().unwrap() {
                    for ring in face["loops"].as_array().unwrap() {
                        for trim in ring["trims"].as_array().unwrap() {
                            if let Some(edge) = trim["edge"].as_u64()
                                && !order.contains(&(edge as usize))
                            {
                                order.push(edge as usize);
                            }
                        }
                    }
                }
                assert_eq!(order.len(), count);
                let mut inverse = vec![0; count];
                for (new, &old) in order.iter().enumerate() {
                    inverse[old] = new;
                }
                let old = definition["edges"].as_array().unwrap().clone();
                definition["edges"] =
                    json!(order.iter().map(|&i| old[i].clone()).collect::<Vec<_>>());
                let old = definition["topology"]["edges"].as_array().unwrap().clone();
                definition["topology"]["edges"] =
                    json!(order.iter().map(|&i| old[i].clone()).collect::<Vec<_>>());
                for face in definition["topology"]["faces"].as_array_mut().unwrap() {
                    for ring in face["loops"].as_array_mut().unwrap() {
                        for trim in ring["trims"].as_array_mut().unwrap() {
                            if let Some(edge) = trim["edge"].as_u64() {
                                trim["edge"] = json!(inverse[edge as usize]);
                            }
                        }
                    }
                }
            } else {
                for value in object.values_mut() {
                    edge_traversal(value);
                }
            }
        }
        _ => {}
    }
}

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
            let mut value = actual.value.clone();
            for diagnostic in ["events", "history", "undo_events", "redo_events"] {
                expected.as_object_mut().unwrap().remove(diagnostic);
            }
            // Complete restoration and source definitions also require native
            // table ordering. Only partial results have the documented edge
            // allocation difference; their entire graph is compared below.
            let partial = actual.id.starts_with("partial-")
                || actual.id.starts_with("band-similar-0-")
                || actual.id.starts_with("history-band-similar-0-");
            if !partial {
                crate::test_json::close(&value, &expected, &actual.id, 1e-9, 0.);
            }
            // Topology, UV scalar intervals, control nets and vertex references
            // remain compared in full after this explicit edge-table permutation.
            edge_traversal(&mut value);
            edge_traversal(&mut expected);
            if std::panic::catch_unwind(|| {
                crate::test_json::close(&value, &expected, &actual.id, 1e-9, 0.)
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
