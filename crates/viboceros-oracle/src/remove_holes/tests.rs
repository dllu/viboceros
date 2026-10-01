use super::*;

#[test]
fn full_definitions_match_public_remove_holes_overloads() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/brep_remove_holes.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/brep_remove_holes.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 28);
    assert_eq!(observed["results"].as_array().unwrap().len(), 28);
    let mut null = 0;
    for (a, b) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        crate::test_json::close(&a.value, &b["value"], &a.id, 1e-9, 0.);
        null += usize::from(a.value["after"].is_null());
    }
    assert_eq!(null, 4);
}

#[test]
fn invalid_indices_and_ambiguous_iterations_fail_without_output() {
    let mut request: ProbeRequest = serde_json::from_value(json!({
        "protocol_version":1,"iterations":1,"operations":[{
            "op":"brep_remove_holes","id":"tube","loops":[[2,1]],
            "source":{"source":{"type":"solid_tube","radii":[2.,5.],"height":8.}}
        }]
    }))
    .unwrap();
    let Operation::BrepRemoveHoles { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    let baseline = run(fixture, Tolerance::DEFAULT).unwrap();
    fixture.loops = Some(vec![(2, 1), (2, 1)]);
    assert_eq!(run(fixture, Tolerance::DEFAULT).unwrap(), baseline);
    for loops in [vec![(4, 0)], vec![(2, 2)], vec![(2, 1); 1001]] {
        fixture.loops = Some(loops);
        assert!(run(fixture, Tolerance::DEFAULT).is_err());
    }
    request.iterations = 2;
    assert!(run_request(&request).is_err());
}
