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
