use super::*;

#[test]
fn actual_component_commands_match_complete_native_geometry_and_object_metadata() {
    for (input, capture, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_holes_history.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_holes_history.json"),
            16,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_holes_components.json"),
            include_str!(
                "../../../../tools/rhino_oracle/observations/untrim_holes_components.json"
            ),
            44,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_holes_undo.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_holes_undo.json"),
            8,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/untrim_holes_limits.json"),
            include_str!("../../../../tools/rhino_oracle/observations/untrim_holes_limits.json"),
            42,
        ),
    ] {
        let request: ProbeRequest = serde_json::from_str(input).unwrap();
        let native: Value = serde_json::from_str(capture).unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), count);
        assert_eq!(native["results"].as_array().unwrap().len(), count);
        for (actual, native) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(actual.id, native["id"]);
            let mut expected = native["value"].clone();
            for diagnostic in ["events", "history", "undo_events", "redo_events"] {
                expected.as_object_mut().unwrap().remove(diagnostic);
            }
            crate::test_json::close(&actual.value, &expected, &actual.id, 1e-9, 0.);
        }
    }
}

#[test]
fn invalid_sequences_and_ambiguous_iterations_are_rejected() {
    let source = json!({"brep":{"source":{"type":"solid_tube","radii":[2.,5.],"height":8.}}});
    let base = json!({"sources":[source],"all":false,"components":[[0,3]],
        "maximum_edge_length":0.,"keep_trim_objects":true,"pick":"mouse","undo_after":[1]});
    let fixture: UntrimHolesFixture = serde_json::from_value(base.clone()).unwrap();
    assert!(run(&fixture, Tolerance::DEFAULT).is_ok());
    for (key, value) in [
        ("sources", json!([])),
        ("components", json!([[1, 3]])),
        ("components", json!([[0, 100]])),
        ("components", json!(vec![[0, 3]; 65])),
        ("maximum_edge_length", json!(-1.)),
        ("undo_after", json!([0])),
        ("undo_after", json!([1, 1])),
        ("undo_after", json!([2])),
        ("pick", json!("preselect")),
    ] {
        let mut invalid = base.clone();
        invalid[key] = value;
        let fixture: UntrimHolesFixture = serde_json::from_value(invalid).unwrap();
        assert!(run(&fixture, Tolerance::DEFAULT).is_err(), "{key}");
    }
    let mut operation = base;
    operation["op"] = json!("untrim_holes_command");
    operation["id"] = json!("holes");
    let request: ProbeRequest = serde_json::from_value(
        json!({"protocol_version":1,"iterations":2,"operations":[operation]}),
    )
    .unwrap();
    assert!(run_request(&request).is_err());

    let mut repeated = json!({"sources":[{"brep":{"source":{"type":"solid_tube","radii":[2.,5.],"height":8.}}}],
        "all":false,"components":[[0,3],[0,3]],"maximum_edge_length":0.,
        "keep_trim_objects":false,"pick":"preselect"});
    let fixture: UntrimHolesFixture = serde_json::from_value(repeated.clone()).unwrap();
    assert!(run(&fixture, Tolerance::DEFAULT).is_ok());
    repeated["pick"] = json!("mouse");
    let fixture: UntrimHolesFixture = serde_json::from_value(repeated).unwrap();
    assert!(run(&fixture, Tolerance::DEFAULT).is_err());
}
