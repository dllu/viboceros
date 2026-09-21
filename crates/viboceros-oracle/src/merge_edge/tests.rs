use super::*;

#[test]
fn scoped_kernel_geometry_matches_command_choices_after_explicit_edge_permutations() {
    use viboceros_geometry::BrepEdgeMergeScope;
    let request: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/diagnostics/merge_edge/mouse-request.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/diagnostics/merge_edge/mouse-response.json"
    ))
    .unwrap();
    let provenance: Value = serde_json::from_str(include_str!(
        "../../../../docs/merge-edge-mouse-provenance.json"
    ))
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = observed["results"].as_array().unwrap();
    assert_eq!(operations.len(), 15);
    assert_eq!(results.len(), 15);
    let mut counts = [0; 5];
    for (operation, result) in operations.iter().zip(results) {
        assert_eq!(operation["id"], result["id"]);
        let id = operation["id"].as_str().unwrap();
        let source: crate::brep_source::BrepSourceFixture =
            serde_json::from_value(operation["sources"][0]["brep"].clone()).unwrap();
        let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
        let source = source.build(tolerance).unwrap();
        let value = &result["value"];
        crate::test_json::close(
            &crate::brep_join::geometry_record(&source, tolerance).unwrap(),
            &value["before"][0]["geometry"]["brep"],
            id,
            1e-9,
            1e-10,
        );
        let (scope, count, removed) = match operation["choice"].as_str().unwrap() {
            "EdgeA" => (BrepEdgeMergeScope::Start, 0, 1),
            "EdgeB" => (BrepEdgeMergeScope::End, 1, 1),
            "Both" => (BrepEdgeMergeScope::Both, 2, 2),
            "All" => (BrepEdgeMergeScope::Chain, 3, 3),
            "Cancel" => {
                counts[4] += 1;
                assert_eq!(value["succeeded"], false);
                assert_eq!(value["history_tested"], false);
                assert_eq!(value["before"], value["after"]);
                continue;
            }
            _ => panic!("unexpected choice"),
        };
        counts[count] += 1;
        let merged = source
            .try_merge_edge_with_scope(
                operation["edge"].as_u64().unwrap() as usize,
                scope,
                value["angular_tolerance"].as_f64().unwrap(),
                tolerance,
            )
            .unwrap();
        assert_eq!(source.edges().len() - merged.edges().len(), removed);
        // The raw command has a different edge-table policy from the kernel.
        // Only these explicit measured permutations are permitted; no fitting,
        // sorting by samples, cyclic trim rotation, or field exclusions.
        let order: Vec<usize> =
            serde_json::from_value(provenance["kernel_edge_permutations"][id].clone()).unwrap();
        let geometry = merged.reordered_edges(&order, tolerance).unwrap();
        crate::test_json::close(
            &crate::brep_join::geometry_record(&geometry, tolerance).unwrap(),
            &value["after"][0]["geometry"]["brep"],
            id,
            1e-9,
            1e-10,
        );
        assert_eq!(value["succeeded"], true);
        assert_eq!(value["history_tested"], true);
        assert_eq!(value["undo"], value["before"]);
        assert_eq!(value["redo"], value["after"]);
    }
    assert_eq!(counts, [3; 5]);
}

#[test]
fn selected_edge_api_replays_full_definitions_with_only_recorded_phantom_knot_differences() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/brep_merge_edge.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/brep_merge_edge.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 45);
    assert_eq!(observed["results"].as_array().unwrap().len(), 45);
    let mut exceptions = 0;
    let mut unchanged = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        let mut native = a.value.clone();
        let mut rhino = b["value"].clone();
        if a.id.starts_with("unclamped-") {
            exceptions += 1;
            // 3DM omits two unused outer knots. Assert their exact recorded
            // disagreement before excluding only those entries, in both states.
            for state in ["before", "after"] {
                for (value, endpoints) in [(&mut native, [-2., 3.]), (&mut rhino, [-1., 2.])] {
                    let knots = value[state]["surfaces"][0]["knots_u"]
                        .as_array_mut()
                        .unwrap();
                    assert_eq!(knots.remove(0), endpoints[0]);
                    assert_eq!(knots.pop().unwrap(), endpoints[1]);
                }
            }
        }
        crate::test_json::close(&native, &rhino, &a.id, 1e-9, 1e-10);
        let removed = a.value["removed"].as_u64().unwrap();
        assert_eq!(a.value["api_return"], removed + 1);
        assert_eq!(b["value"]["api_return"], removed + 1);
        if removed == 0 {
            unchanged += 1;
            assert_eq!(a.value["before"], a.value["after"]);
        } else {
            assert_eq!(removed, 3);
        }
    }
    assert_eq!(exceptions, 5);
    assert_eq!(unchanged, 9);
}

#[test]
fn invalid_index_and_angle_produce_failures_not_fabricated_geometry() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/brep_merge_edge.json"
    ))
    .unwrap();
    let Operation::BrepMergeEdge { fixture, .. } = &request.operations[0] else {
        panic!()
    };
    for edge in [usize::MAX, 18] {
        let mut fixture = fixture.clone();
        fixture.edge = edge;
        assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    }
    for angle in [-1., f64::NAN, f64::INFINITY, std::f64::consts::PI.next_up()] {
        let mut fixture = fixture.clone();
        fixture.angle = angle;
        assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    }
}
