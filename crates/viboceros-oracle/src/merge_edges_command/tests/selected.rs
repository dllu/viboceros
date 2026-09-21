use super::*;

/// An explicit permutation changes only edge-table order and index references.
/// No approximate geometry matching or dropped numeric fields are permitted.
fn permute_record(brep: &mut Value, order: &[usize]) {
    let mut inverse = vec![usize::MAX; order.len()];
    for (i, &old) in order.iter().enumerate() {
        assert!(old < order.len());
        assert_eq!(inverse[old], usize::MAX);
        inverse[old] = i;
    }
    for field in ["edges", "edge_tolerances"] {
        let values = brep[field].as_array().unwrap();
        assert_eq!(values.len(), order.len());
        brep[field] = json!(order.iter().map(|&i| &values[i]).collect::<Vec<_>>());
    }
    // cap_command::geometry_record orders this *incidence witness* by edge
    // indices, independently of the ordered surface and UV trim definitions.
    // Reapply exactly that serializer order after renumbering, preserving the
    // corresponding area witness. Ordered trim_curves are never rotated/sorted.
    type Loop = (String, Vec<(Option<usize>, bool)>);
    let mut faces: Vec<(Vec<Loop>, f64)> = serde_json::from_value(brep["faces"].clone()).unwrap();
    for (loops, _) in &mut faces {
        for (_, edges) in loops.iter_mut() {
            for (edge, _) in edges.iter_mut() {
                *edge = edge.map(|old| inverse[old]);
            }
            edges.sort();
        }
        loops.sort();
    }
    faces.sort_by(|a, b| a.0.cmp(&b.0));
    brep["faces"] = json!(faces);
}

#[test]
fn actual_selected_command_replays_geometry_attributes_selection_and_history() {
    let batches = [
        (
            include_str!(
                "../../../../../tools/rhino_oracle/diagnostics/merge_edge/mouse-request.json"
            ),
            include_str!(
                "../../../../../tools/rhino_oracle/diagnostics/merge_edge/mouse-response.json"
            ),
            include_str!("../../../../../docs/merge-edge-mouse-provenance.json"),
            15,
        ),
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/merge_edge_command.json"),
            include_str!("../../../../../tools/rhino_oracle/observations/merge_edge_command.json"),
            include_str!("../../../../../docs/merge-edge-command-provenance.json"),
            21,
        ),
    ];
    let mut angles = 0;
    for (request, observed, metadata, count) in batches {
        let request: ProbeRequest = serde_json::from_str(request).unwrap();
        let observed: Value = serde_json::from_str(observed).unwrap();
        let metadata: Value = serde_json::from_str(metadata).unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), count);
        assert_eq!(observed["results"].as_array().unwrap().len(), count);
        for ((actual, expected), operation) in actual
            .results
            .iter()
            .zip(observed["results"].as_array().unwrap())
            .zip(&request.operations)
        {
            assert_eq!(actual.id, expected["id"]);
            let mut value = actual.value.clone();
            if let Some(order) = metadata["kernel_edge_permutations"].get(&actual.id) {
                let order: Vec<usize> = serde_json::from_value(order.clone()).unwrap();
                for phase in ["after", "redo"] {
                    permute_record(&mut value[phase][0]["geometry"]["brep"], &order);
                }
            }
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
            close(&value, &reference, &actual.id);
            if actual.id.starts_with("planar-kink-") {
                angles += 1;
                let Operation::MergeEdgeCommand { fixture, .. } = operation else {
                    panic!()
                };
                let tolerance =
                    Tolerance::try_new(1e-9, 1e-12, fixture.base.angular_tolerance.unwrap())
                        .unwrap();
                let mut doc = Document::new(tolerance);
                let id = doc
                    .add_geometry(fixture.base.sources[0].geometry(tolerance).unwrap())
                    .unwrap();
                let selection =
                    viboceros_command::MergeEdgeSelection::prepare(&doc, id, fixture.edge).unwrap();
                assert_eq!(
                    !selection.choices().is_empty(),
                    expected["value"]["command_history"]
                        .as_str()
                        .unwrap()
                        .contains("Choose option"),
                    "{}",
                    actual.id
                );
            }
        }
    }
    assert_eq!(angles, 9);
}
