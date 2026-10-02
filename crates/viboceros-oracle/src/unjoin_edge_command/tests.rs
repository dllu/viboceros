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

#[test]
fn native_modifier_and_rectangle_sequences_match_each_selection_step_and_full_history() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/unjoin_edge_selection.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/unjoin_edge_selection.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 42);
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
fn invalid_sequences_fail_before_geometry_history_is_applied() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/unjoin_edge_selection.json"
    ))
    .unwrap();
    let base = fixture["operations"][0].clone();
    for (key, value) in [
        ("steps", json!([])),
        ("steps", json!([{"kind":"key","value":"None"}])),
        (
            "steps",
            json!([{"kind":"key","value":"None"},{"kind":"key","value":"Undo"}]),
        ),
        (
            "steps",
            json!(vec![
                json!({"kind":"click","component":[0,2],"modifiers":"plain"});
                65
            ]),
        ),
        (
            "steps",
            json!([{"kind":"click","component":[1,2],"modifiers":"plain"}]),
        ),
        (
            "steps",
            json!([{"kind":"click","component":[0,999],"modifiers":"plain"}]),
        ),
        (
            "steps",
            json!([{"kind":"window","corners":[[1e7,0.,0.],[0.,1.,0.]],"modifiers":"plain"}]),
        ),
        (
            "steps",
            json!([{"kind":"window","corners":[[0.,0.,0.],[1.,1.,0.]],"modifiers":"plain"}]),
        ),
        ("object_preselect", json!(true)),
        ("components", json!([[0, 2]])),
        ("pick", json!("mouse")),
    ] {
        let mut operation = base.clone();
        operation[key] = value;
        if key == "steps"
            && operation["steps"][0]["kind"] == "window"
            && operation["steps"][0]["corners"][0][0] == 0.
        {
            operation["sources"][0]["brep"]["source"] =
                json!({"type":"box","min":[0.,0.,0.],"max":[2.,3.,5.]});
        }
        let request: ProbeRequest = serde_json::from_value(
            json!({"protocol_version":1,"iterations":1,"operations":[operation]}),
        )
        .unwrap();
        assert!(run_request(&request).is_err(), "{key}");
    }
    for steps in [
        json!([{"kind":"key","value":"_Delete"}]),
        json!([{"kind":"click","component":[0,2],"modifiers":"invalid"}]),
        json!([{"kind":"click","component":[0,2],"modifiers":"plain","expected":[2]}]),
    ] {
        let mut operation = base.clone();
        operation["steps"] = steps;
        assert!(
            serde_json::from_value::<ProbeRequest>(
                json!({"protocol_version":1,"iterations":1,"operations":[operation]})
            )
            .is_err()
        );
    }
}
