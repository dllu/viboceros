use super::*;

fn close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-9_f64.max(1e-10 * a.abs().max(b.abs())),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                close(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn cap_replays_rhino_spatial_edges_oriented_incidence_integrals_and_document_state() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/cap.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 109);
    assert_eq!(expected["results"].as_array().unwrap().len(), 109);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(&a.value, &b["value"], &a.id);
        if a.id.starts_with("smooth-ellipse-") {
            // Independent 70-digit quadrature of |ellipse'(t) x extrusion|.
            // Rhino's integral is about 1.98e-9 higher; retain both raw values.
            let area = a.value["input"]["faces"][0][1].as_f64().unwrap();
            assert!((area - 67.13086695243027).abs() < 1e-12);
        }
    }
}

#[test]
fn kinked_boundaries_replay_complete_rhino_topology_and_document_state() {
    replay(
        include_str!("../../../../tools/rhino_oracle/fixtures/cap_kink_boundaries.json"),
        include_str!("../../../../tools/rhino_oracle/observations/cap_kink_boundaries.json"),
        4,
    );
}

#[test]
fn cap_preserves_weighted_and_high_degree_segments_and_uses_geometric_kink_angles() {
    replay(
        include_str!("../../../../tools/rhino_oracle/fixtures/cap_edge_splits.json"),
        include_str!("../../../../tools/rhino_oracle/observations/cap_edge_splits.json"),
        12,
    );
}

#[test]
fn cap_kink_cutoff_is_independent_of_document_angle_and_edge_table_order() {
    replay(
        include_str!("../../../../tools/rhino_oracle/fixtures/cap_edge_splits_angular.json"),
        include_str!("../../../../tools/rhino_oracle/observations/cap_edge_splits_angular.json"),
        15,
    );
}

#[test]
fn large_parameter_origin_retains_rhino_topology_and_integral_discrepancies() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap_parameter_origin.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/cap_parameter_origin.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 1);
    assert_eq!(expected["results"].as_array().unwrap().len(), 1);
    let mut native = actual.results[0].value.clone();
    let mut rhino = expected["results"][0]["value"].clone();
    assert_eq!(actual.results[0].id, expected["results"][0]["id"]);
    let a = native["objects"][0]
        .as_object_mut()
        .unwrap()
        .remove("geometry")
        .unwrap();
    let b = rhino["objects"][0]
        .as_object_mut()
        .unwrap()
        .remove("geometry")
        .unwrap();
    let area_a = native["input"]["faces"][0][1].take().as_f64().unwrap();
    let area_b = rhino["input"]["faces"][0][1].take().as_f64().unwrap();
    close(&native, &rhino, "input-except-area-and-document");
    // Independent sum of parallelogram areas |triangle_segment x extrusion|.
    let analytic = 4. * 29_f64.sqrt() + 746_f64.sqrt() + 3. * 26_f64.sqrt();
    assert!((area_a - analytic).abs() < 1e-12);
    assert!((area_b - analytic).abs() > 3e-7);
    assert_eq!(a["edges"].as_array().unwrap().len(), 7);
    assert_eq!(b["edges"].as_array().unwrap().len(), 5);
    assert_eq!(a["vertices"].as_array().unwrap().len(), 6);
    assert_eq!(b["vertices"].as_array().unwrap().len(), 4);
    assert_eq!(a["edges"][0]["curve"]["domain"], json!([1e9, 1e9 + 7.]));
    assert_eq!(b["edges"][0]["curve"]["domain"], json!([1e9, 1e9 + 11.]));
    for g in [&a, &b] {
        assert_eq!(g["solid"], true);
        assert_eq!(g["faces"].as_array().unwrap().len(), 3);
    }
    assert!((a["volume"].as_f64().unwrap() - 30.).abs() < 1e-12);
    assert!((b["volume"].as_f64().unwrap() - 30.).abs() > 1e-7);
}

#[test]
fn all_negative_weights_retain_the_observed_rhino_noop_discrepancy() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap_negative_weights.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/cap_negative_weights.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 1);
    assert_eq!(expected["results"].as_array().unwrap().len(), 1);
    let mut native = actual.results[0].value.clone();
    let mut rhino = expected["results"][0]["value"].clone();
    assert_eq!(actual.results[0].id, expected["results"][0]["id"]);
    let native_geometry = native["objects"][0]
        .as_object_mut()
        .unwrap()
        .remove("geometry")
        .unwrap();
    let rhino_geometry = rhino["objects"][0]
        .as_object_mut()
        .unwrap()
        .remove("geometry")
        .unwrap();
    // Keep the full differing records: neither normalize weight signs nor
    // discard input geometry/document state to make the comparison pass.
    close(&native, &rhino, "input-and-document");
    close(&rhino_geometry, &rhino["input"], "rhino-noop");
    assert_eq!(rhino_geometry["solid"], false);
    assert_eq!(rhino_geometry["faces"].as_array().unwrap().len(), 1);
    assert_eq!(native_geometry["solid"], true);
    assert_eq!(native_geometry["faces"].as_array().unwrap().len(), 3);
    assert_eq!(native_geometry["edges"].as_array().unwrap().len(), 7);
    close(&native_geometry["volume"], &json!(30.), "native-volume");
}

fn replay(request: &str, expected: &str, count: usize) {
    let request: ProbeRequest = serde_json::from_str(request).unwrap();
    let expected: Value = serde_json::from_str(expected).unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), count);
    assert_eq!(expected["results"].as_array().unwrap().len(), count);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn cap_shared_artifact_cannot_overwrite_existing_files() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap.json"
    ))
    .unwrap();
    let Operation::CapCommand { mut fixture, .. } = request.operations[0].clone() else {
        panic!("cap fixture")
    };
    let temporary = OracleTemporaryFile::new("cap-existing");
    std::fs::write(&temporary.path, b"owned sentinel").unwrap();
    fixture.artifact_path = Some(temporary.path.to_str().unwrap().into());
    assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    assert_eq!(std::fs::read(&temporary.path).unwrap(), b"owned sentinel");
}
