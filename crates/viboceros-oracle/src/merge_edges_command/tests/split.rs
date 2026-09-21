use super::*;

#[test]
fn split_edge_replays_complete_rhino_geometry_selection_attributes_and_history() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/split_edge_command.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/split_edge_command.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 21);
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 21);
    assert_eq!(observed["results"].as_array().unwrap().len(), 21);
    for (actual, expected) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(actual.id, expected["id"]);
        let mut reference = expected["value"].clone();
        for key in [
            "command_events",
            "command_history",
            "undo_events",
            "redo_events",
            "undo_event_snapshot",
            "redo_event_snapshot",
        ] {
            reference.as_object_mut().unwrap().remove(key);
        }
        // No topology permutations, sorting, fitted geometry, or omitted numeric fields.
        close(&actual.value, &reference, &actual.id);
    }
}

#[test]
fn split_edge_distance_replays_open_closed_typed_mouse_and_history_records() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/split_edge_distance_command.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/split_edge_distance_command.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 19);
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 19);
    assert_eq!(observed["results"].as_array().unwrap().len(), 19);
    for (actual, expected) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(actual.id, expected["id"]);
        let mut reference = expected["value"].clone();
        for key in [
            "command_events",
            "command_history",
            "undo_events",
            "redo_events",
            "undo_event_snapshot",
            "redo_event_snapshot",
        ] {
            reference.as_object_mut().unwrap().remove(key);
        }
        // Rhino's curved arc-length inversion has independently measured residuals
        // above 1e-9. Retain every numeric field and use a separate explicit bound
        // for these records; never weaken the earlier unconstrained fixtures.
        let epsilon = if actual.id.starts_with("box-") {
            1e-9
        } else {
            1e-6
        };
        crate::test_json::close(&actual.value, &reference, &actual.id, epsilon, 1e-10);
    }
}

#[test]
fn distance_fixture_rejects_ambiguous_and_oversized_input_grammars() {
    for extra in [
        json!({"parameters":null,"inputs":[]}),
        json!({"parameters":[],"inputs":null}),
    ] {
        let mut fixture = json!({"sources":[],"edge":0,"pick":"mouse"});
        fixture
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(serde_json::from_value::<SplitEdgeFixture>(fixture).is_err());
    }
    for extra in [
        json!({}),
        json!({"parameters":[],"inputs":[]}),
        json!({"inputs":vec![json!({"distance":1});65]}),
        json!({"inputs":[{"pick":{"point":[2,0,0],"osnap":"Point","offset":[33,0]}}]}),
        json!({"record_viewport":true,"inputs":[{"point":1}]}),
        json!({"persistent_snaps":["Cen","Cen"],"inputs":[{"point":1}]}),
        json!({"persistent_snaps":["NoSnap"],"inputs":[{"point":1}]}),
        json!({"inputs":[{"pick":{"point":[2,0,0],"osnap":"Persistent"}}]}),
    ] {
        let mut fixture = json!({"sources":[{"brep":{"source":{"type":"box","min":[0,0,0],"max":[10,12,14]}}}],"edge":0,"pick":"mouse"});
        fixture
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let fixture: SplitEdgeFixture = serde_json::from_value(fixture).unwrap();
        assert!(matches!(
            run_split(&fixture, Tolerance::DEFAULT),
            Err(ProbeError::FixtureInvariant(
                "invalid SplitEdge command fixture"
            ))
        ));
    }
    for step in [
        json!({"point":1,"distance":2}),
        json!({"mouse":true}),
        json!({"command":"_Delete"}),
        json!({"pick":{"point":[2,0,0],"osnap":"Point _Delete"}}),
        json!({"pick":{"point":[2,0],"osnap":"Point"}}),
        json!({"pick":{"point":[true,0,0],"osnap":"Point"}}),
        json!({"pick":{"point":[2,0,0],"osnap":"Point","offset":[0.5,0]}}),
        json!({"pick":{"point":[2,0,0],"osnap":"Point","unexpected":true}}),
        json!({"pick":{"point":[2,0,0],"aim":null,"osnap":"Cen"}}),
        json!({"pick":{"point":[2,0,0],"aim":[true,0,0],"osnap":"Cen"}}),
    ] {
        let fixture = json!({"sources":[],"edge":0,"pick":"mouse","inputs":[step]});
        assert!(serde_json::from_value::<SplitEdgeFixture>(fixture).is_err());
    }
}

#[test]
fn split_edge_snaps_replay_complete_geometry_and_history_without_faking_screen_controls() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/split_edge_snaps_command.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/split_edge_snaps_command.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 17);
    assert_eq!(observed["results"].as_array().unwrap().len(), 17);
    let (mut replayed, mut controls) = (0, 0);
    for (operation, expected) in request
        .operations
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        let Operation::SplitEdgeCommand { id, fixture } = operation else {
            panic!()
        };
        assert_eq!(id, expected["id"].as_str().unwrap());
        if id.ends_with("-nosnap") {
            assert!(matches!(
                run_split(fixture, Tolerance::DEFAULT),
                Err(ProbeError::FixtureInvariant(
                    "NoSnap screen controls require recorded viewport calibration"
                ))
            ));
            controls += 1;
            continue;
        }
        let (actual, _) = run_split(fixture, Tolerance::DEFAULT).unwrap();
        let mut reference = expected["value"].clone();
        for key in [
            "command_events",
            "command_history",
            "undo_events",
            "redo_events",
            "undo_event_snapshot",
            "redo_event_snapshot",
        ] {
            reference.as_object_mut().unwrap().remove(key);
        }
        close(&actual, &reference, id);
        replayed += 1;
    }
    assert_eq!((replayed, controls), (15, 2));
}
