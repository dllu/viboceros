use super::*;

#[test]
fn permanent_arrays_check_counts_original_selection_domains_and_group_memberships() {
    for (text, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/plane_arrays.json"),
            64,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/surface_array_bounds.json"),
            32,
        ),
    ] {
        check_arrays(text, count);
    }
}

fn check_arrays(text: &str, count: usize) {
    let request: ProbeRequest = serde_json::from_str(text).unwrap();
    let response = run_request(&request).unwrap();
    assert_eq!(response.results.len(), count);
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
            let (domain, count) = if let Some(curve) = source.curve_ref() {
                let domain = curve.domain();
                (json!([*domain.start(), *domain.end()]), 33)
            } else if let Geometry::NurbsSurface(s) = source {
                let u = s.domain_u();
                let v = s.domain_v();
                (json!([[*u.start(), *u.end()], [*v.start(), *v.end()]]), 25)
            } else {
                panic!("array source")
            };
            assert_eq!(record["domain"], domain);
            assert_eq!(record["points"].as_array().unwrap().len(), count);
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

#[test]
fn permanent_surface_bounds_contain_diagnostic_samples_and_preserve_negative_gauges() {
    for (text, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/surface_bounds.json"),
            25,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/surface_bounds_diagnostics.json"),
            4,
        ),
    ] {
        let request: ProbeRequest = serde_json::from_str(text).unwrap();
        let response = run_request(&ProbeRequest {
            iterations: 1,
            ..request
        })
        .unwrap();
        assert_eq!(response.results.len(), count);
        for result in response.results {
            for axis in 0..3 {
                let min = result.value["min"][axis].as_f64().unwrap();
                let max = result.value["max"][axis].as_f64().unwrap();
                assert!(
                    min.is_finite() && max.is_finite() && min <= max,
                    "{}",
                    result.id
                );
                if let Some(samples) = result.value.get("sample_bounds") {
                    assert!(
                        min <= samples["min"][axis].as_f64().unwrap() + 1e-8,
                        "{}",
                        result.id
                    );
                    assert!(
                        max >= samples["max"][axis].as_f64().unwrap() - 1e-8,
                        "{}",
                        result.id
                    );
                }
            }
            if matches!(
                result.id.as_str(),
                "surface-bounds-quad" | "surface-bounds-quad-negative-gauge"
            ) {
                assert_eq!(result.value["min"], json!([0., 0., 0.]));
                assert_eq!(result.value["max"], json!([1., 1., 3.]));
            }
        }
    }
}
