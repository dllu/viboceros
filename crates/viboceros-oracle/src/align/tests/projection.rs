use super::*;

fn compare(a: &Value, b: &Value) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-8,
            "{a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                compare(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for (key, a) in a {
                compare(a, &b[key]);
            }
        }
        _ => assert_eq!(a, b),
    }
}

#[test]
fn projection_modes_match_all_recorded_geometry_attributes_and_selection() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/align_projection.json"
    ))
    .unwrap();
    let rhino: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/align_projection.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 34);
    assert_eq!(rhino["results"].as_array().unwrap().len(), 34);
    for (a, b) in actual
        .results
        .iter()
        .zip(rhino["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"]);
    }
}

#[test]
fn raw_mesh_discrepancies_are_retained_and_explained_by_single_precision_storage() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/align_projection_mesh_diagnostics.json"
    ))
    .unwrap();
    let rhino: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/align_projection_mesh_diagnostics.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 4);
    assert_eq!(rhino["results"].as_array().unwrap().len(), 4);
    for (mut a, b) in actual
        .results
        .into_iter()
        .zip(rhino["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        let mut max_error = 0_f64;
        for (p, q) in a.value["objects"][3]["points"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(b["value"]["objects"][3]["points"].as_array().unwrap())
        {
            for i in 0..3 {
                let raw = p[i].as_f64().unwrap();
                let expected = q[i].as_f64().unwrap();
                max_error = max_error.max((raw - expected).abs());
                assert_eq!(f64::from(raw as f32), expected, "{} mesh coordinate", a.id);
                p[i] = json!(f64::from(raw as f32));
            }
        }
        assert!(
            max_error > 1e-8,
            "raw discrepancy unexpectedly disappeared: {}",
            a.id
        );
        // Compare every remaining field unmodified, including all nonmesh
        // coordinates. This diagnostic does not change the raw oracle output.
        compare(&a.value, &b["value"]);
    }
}
