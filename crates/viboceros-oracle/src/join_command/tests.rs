use super::*;
mod surfaces;

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

#[test]
fn closed_cycles_replay_raw_seams_local_domains_and_command_boundary_selection() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/join_cycles.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/join_cycles.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let records = expected["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 284);
    assert_eq!(records.len(), 284);
    for (a, b) in actual.results.iter().zip(records) {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn rational_and_composite_encodings_replay_every_raw_rhino_field() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/join_encodings.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/join_encodings.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let records = expected["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 352);
    assert_eq!(records.len(), 352);
    for (a, b) in actual.results.iter().zip(records) {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn weighted_copy_seams_replay_midpoint_tolerance_and_composite_closure_boundaries() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/join_weight_seams.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/join_weight_seams.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let records = expected["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 192);
    assert_eq!(records.len(), 192);
    for (a, b) in actual.results.iter().zip(records) {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn endpoint_search_replays_distant_outliers_without_losing_nearby_joins() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/join_search.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/join_search.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let records = expected["results"].as_array().unwrap();
    assert_eq!(actual.results.len(), 48);
    assert_eq!(records.len(), 48);
    for (a, b) in actual.results.iter().zip(records) {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
        for (index, object) in b["value"]["objects"].as_array().unwrap().iter().enumerate() {
            if object["curve"]["samples"][0]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_f64().unwrap().abs() > 1e12)
            {
                // A relative epsilon must not hide movement of the distant,
                // untouched source. Its entire record must agree exactly.
                assert_eq!(a.value["objects"][index], *object, "{}", a.id);
            }
        }
    }
}

#[test]
fn saved_events_distinguish_early_completion_and_nothing_from_macro_success() {
    let recorded: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/join_command_events.json"
    ))
    .unwrap();
    let rows = recorded["results"].as_array().unwrap();
    assert_eq!(rows.len(), 8);
    for command in ["Join", "JoinCopy"] {
        let id = format!("closed_chain_{command}_False");
        let value = &rows.iter().find(|r| r["id"] == id).unwrap()["value"];
        let events = value["command_events"].as_array().unwrap();
        let index = events.iter().position(|e| e["name"] == command).unwrap();
        assert_eq!(events[index]["objects"], value["objects"]);
        assert_eq!(events[index + 1]["name"], "SelID");
        assert_eq!(events[index + 1]["selected"], json!([3]));
        assert_ne!(events[index]["selected"], events[index + 1]["selected"]);
    }
    let value = &rows
        .iter()
        .find(|r| r["id"] == "single_Join_post_False")
        .unwrap()["value"];
    assert_eq!(value["succeeded"], false);
    assert_eq!(
        value["command_events"].as_array().unwrap().last().unwrap()["result"],
        "Nothing"
    );
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
