use super::*;

#[test]
fn retained_area_centroid_replay_preserves_command_and_precision_differences() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/area_centroid.json"
    ))
    .unwrap();
    let observed: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/area_centroid.json"
    ))
    .unwrap();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/area_centroid_integrals.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 54);
    let mut differences = Vec::new();
    let mut tight_differences = Vec::new();
    let close = |a: &Value, b: &Value| (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-9;
    let mass_close = |a: &Value, b: &Value| {
        if a.is_null() || b.is_null() {
            return a == b;
        }
        close(&a["area"], &b["area"]) && (0..3).all(|i| close(&a["centroid"][i], &b["centroid"][i]))
    };
    for (a, b) in actual
        .results
        .iter()
        .zip(observed["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"].as_str().unwrap());
        let (x, y) = (&a.value, &b["value"]);
        assert_eq!(x["succeeded"], y["succeeded"], "{}", a.id);
        assert_eq!(x["selected"], y["selected"], "{}", a.id);
        let (points, expected) = (
            x["points"].as_array().unwrap(),
            y["points"].as_array().unwrap(),
        );
        assert_eq!(points.len(), expected.len());
        let mut matches = true;
        for (p, q) in points.iter().zip(expected) {
            for field in ["selected", "current_layer", "groups", "name"] {
                assert_eq!(p[field], q[field]);
            }
            matches &= (0..3).all(|i| close(&p["point"][i], &q["point"][i]));
        }
        let masses = x["properties"].as_array().unwrap();
        let default = y["properties"].as_array().unwrap();
        let tight = y["tight_properties"].as_array().unwrap();
        assert_eq!(masses.len(), default.len());
        assert_eq!(masses.len(), tight.len());
        matches &= masses.iter().zip(default).all(|(a, b)| mass_close(a, b));
        if !matches {
            differences.push(a.id.as_str());
        }
        if !masses.iter().zip(tight).all(|(a, b)| mass_close(a, b)) {
            tight_differences.push(a.id.as_str());
        }
        if let Some(expected) = reference["values"].get(&a.id) {
            assert!(
                (masses[0]["area"].as_f64().unwrap()
                    - expected["area"].as_str().unwrap().parse::<f64>().unwrap())
                .abs()
                    < 1e-11
            );
            for i in 0..3 {
                assert!(
                    (masses[0]["centroid"][i].as_f64().unwrap()
                        - expected["centroid"][i]
                            .as_str()
                            .unwrap()
                            .parse::<f64>()
                            .unwrap())
                    .abs()
                        < 1e-11,
                    "{}",
                    a.id
                );
            }
        }
    }
    assert_eq!(
        differences,
        [
            "shape-4",
            "mixed-group",
            "brep-paraboloid-disk",
            "brep-paraboloid-annulus",
            "brep-paraboloid-thin-annulus",
            "brep-paraboloid-capped-outward",
            "brep-paraboloid-capped-inward",
            "brep-paraboloid-rotated-translated"
        ]
    );
    assert_eq!(tight_differences, ["shape-4", "brep-paraboloid-disk"]);
}

#[test]
fn area_centroid_probe_rejects_ambiguous_iterations_and_invalid_source_indices() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/area_centroid.json"
    ))
    .unwrap();
    request.iterations = 2;
    assert!(run_request(&request).is_err());
    request.iterations = 1;
    let Operation::AreaCentroidCommand { fixture, .. } = &mut request.operations[0] else {
        panic!()
    };
    fixture.selected = Some(vec![usize::MAX]);
    assert!(run_request(&request).is_err());
}
