use super::*;

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
