use super::*;

fn close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-9_f64.max(1e-10 * a.abs().max(b.abs())),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                close(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn cap_replays_rhino_spatial_edges_oriented_incidence_integrals_and_document_state() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/cap.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 109);
    assert_eq!(expected["results"].as_array().unwrap().len(), 109);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(&a.value, &b["value"], &a.id);
        if a.id.starts_with("smooth-ellipse-") {
            // Independent 70-digit quadrature of |ellipse'(t) x extrusion|.
            // Rhino's integral is about 1.98e-9 higher; retain both raw values.
            let area = a.value["input"]["faces"][0][1].as_f64().unwrap();
            assert!((area - 67.13086695243027).abs() < 1e-12);
        }
    }
}

#[test]
fn retained_kink_splitting_gaps_preserve_mass_and_document_state_but_not_topology() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap_topology_gaps.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/cap_topology_gaps.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 4);
    assert_eq!(expected["results"].as_array().unwrap().len(), 4);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
        close(&a.value["input"], &b["value"]["input"], &a.id);
        let mut a = a.value.clone();
        let mut b = b["value"].clone();
        let native = a["objects"][0]
            .as_object_mut()
            .unwrap()
            .remove("geometry")
            .unwrap();
        let rhino = b["objects"][0]
            .as_object_mut()
            .unwrap()
            .remove("geometry")
            .unwrap();
        close(&a, &b, "document");
        for field in ["solid", "manifold", "volume"] {
            close(&native[field], &rhino[field], field);
        }
        let area = |g: &Value| {
            g["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f[1].as_f64().unwrap())
                .sum::<f64>()
        };
        close(&json!(area(&native)), &json!(area(&rhino)), "area");
        assert_eq!(native["vertices"].as_array().unwrap().len(), 2);
        assert_eq!(native["edges"].as_array().unwrap().len(), 3);
        assert_eq!(native["faces"].as_array().unwrap().len(), 3);
        assert!(rhino["vertices"].as_array().unwrap().len() > 2);
        assert!(rhino["edges"].as_array().unwrap().len() > 3);
        // Insertion disables kinky-surface splitting: Cap splits the kinked
        // boundary edges, not the retained extrusion wall's underlying face.
        assert_eq!(rhino["faces"].as_array().unwrap().len(), 3);
    }
}

#[test]
fn cap_shared_artifact_cannot_overwrite_existing_files() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/cap.json"
    ))
    .unwrap();
    let Operation::CapCommand { mut fixture, .. } = request.operations[0].clone() else {
        panic!("cap fixture")
    };
    let temporary = OracleTemporaryFile::new("cap-existing");
    std::fs::write(&temporary.path, b"owned sentinel").unwrap();
    fixture.artifact_path = Some(temporary.path.to_str().unwrap().into());
    assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    assert_eq!(std::fs::read(&temporary.path).unwrap(), b"owned sentinel");
}
