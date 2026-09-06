use super::*;

#[test]
fn permanent_arrays_check_counts_original_selection_domains_and_group_memberships() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/plane_arrays.json"
    ))
    .unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), 64);
    for (operation, result) in request.operations.iter().zip(response.results) {
        let Operation::PlaneArray { fixture, .. } = operation else {
            panic!("array fixture")
        };
        let records = result.value["objects"].as_array().unwrap();
        assert_eq!(
            records.iter().filter(|v| v["original"] == true).count(),
            fixture.sources.len()
        );
        for record in records {
            assert_eq!(record["selected"], record["original"]);
            let index = record["source"].as_u64().unwrap() as usize;
            let source = fixture.sources[index].geometry().unwrap();
            let domain = source.as_ref().domain();
            assert_eq!(record["domain"], json!([*domain.start(), *domain.end()]));
            assert_eq!(record["points"].as_array().unwrap().len(), 33);
        }
        let input_groups = fixture
            .groups
            .clone()
            .unwrap_or_else(|| vec![(0..fixture.sources.len()).collect()]);
        let instances = if fixture.sources.len() == 1 {
            1
        } else {
            records.len() / fixture.sources.len()
        };
        let mut expected = (0..instances)
            .flat_map(|_| input_groups.clone())
            .collect::<Vec<_>>();
        for group in &mut expected {
            group.sort();
        }
        expected.sort();
        assert_eq!(result.value["groups"], json!(expected), "{}", result.id);
    }
}

#[test]
fn permanent_bounds_are_finite_and_negative_common_gauges_preserve_the_curve() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/curve_bounds.json"
    ))
    .unwrap();
    let response = run_request(&ProbeRequest {
        iterations: 1,
        ..request
    })
    .unwrap();
    assert_eq!(response.results.len(), 16);
    for result in &response.results {
        for axis in 0..3 {
            assert!(
                result.value["min"][axis].as_f64().unwrap()
                    <= result.value["max"][axis].as_f64().unwrap()
            );
        }
    }
    let diagnostic: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/curve_bounds_diagnostics.json"
    ))
    .unwrap();
    let negative = run_request(&diagnostic).unwrap();
    let positive = response
        .results
        .iter()
        .find(|r| r.id == "bounds-polynomial")
        .unwrap();
    assert_eq!(negative.results[0].value, positive.value);
}
