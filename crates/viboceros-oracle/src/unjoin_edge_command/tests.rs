use super::*;
#[test]
fn native_commands_match_full_geometry_metadata_and_external_history() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/unjoin_edge_command.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/unjoin_edge_command.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 48);
    for (actual, native) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(actual.id, native["id"]);
        let mut expected = native["value"].clone();
        for field in ["events", "history", "undo_events", "redo_events"] {
            expected.as_object_mut().unwrap().remove(field);
        }
        crate::test_json::close(&actual.value, &expected, &actual.id, 1e-9, 0.);
    }
}
#[test]
fn invalid_components_and_repeated_command_iterations_fail_before_execution() {
    let base = json!({"op":"unjoin_edge_command","id":"test","sources":[{"brep":{"source":{"type":"box","min":[0.,0.,0.],"max":[2.,3.,5.]}}}],"components":[[0,0]],"finish":"Enter","undo_redo":true});
    for (key, value) in [
        ("sources", json!([])),
        ("components", json!([[1, 0]])),
        ("components", json!([[0, 12]])),
        ("components", json!(vec![[0, 0]; 100_001])),
        ("kind", json!("face")),
    ] {
        let mut operation = base.clone();
        operation[key] = value;
        if key == "kind" {
            operation["pick"] = json!("mouse");
        }
        let request: ProbeRequest = serde_json::from_value(
            json!({"protocol_version":1,"iterations":1,"operations":[operation]}),
        )
        .unwrap();
        assert!(run_request(&request).is_err(), "{key}");
    }
    let request: ProbeRequest =
        serde_json::from_value(json!({"protocol_version":1,"iterations":2,"operations":[base]}))
            .unwrap();
    assert!(run_request(&request).is_err());
}
