use super::*;

fn compare(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-10_f64.max(1e-12 * a.abs().max(b.abs())),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

#[test]
fn join_and_copy_replay_curve_mesh_geometry_and_complete_document_state() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/join_workflow.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/join_workflow.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 140);
    let expected = expected["results"].as_array().unwrap();
    assert_eq!(expected.len(), 140);
    for (a, b) in actual.results.iter().zip(expected) {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

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
    let Operation::JoinCommand { fixture, .. } = request().operations.remove(0) else {
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
