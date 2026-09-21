use super::*;

#[test]
fn fixture_rejects_missing_sources_mismatched_artifacts_and_invalid_pairs() {
    for fixture in [
        json!({"sources":[],"pairs":[],"join_tolerance":0.}),
        json!({"sources":[{"source":{"type":"box","min":[0,0,0],"max":[2,3,5]}}],"pairs":[],"join_tolerance":0.,"artifact_paths":[]}),
        json!({"sources":[{"source":{"type":"box","min":[0,0,0],"max":[2,3,5]}}],"pairs":[[0,1,false]],"join_tolerance":0.}),
    ] {
        let fixture: BrepJoinFixture = serde_json::from_value(fixture).unwrap();
        assert!(run(&fixture, Tolerance::DEFAULT).is_err());
    }
}

fn close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-10_f64.max(1e-12 * a.abs().max(b.abs())),
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
        _ => assert_eq!(a, b, "{path}"),
    }
}

fn responses(request: &str, expected: &str, count: usize) -> (ProbeResponse, Value) {
    let request: ProbeRequest = serde_json::from_str(request).unwrap();
    let expected: Value = serde_json::from_str(expected).unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), count);
    assert_eq!(expected["results"].as_array().unwrap().len(), count);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"]);
    }
    (actual, expected)
}

#[test]
fn full_edge_assemblies_replay_raw_rhino_topology_geometry_and_integrals() {
    let (actual, expected) = responses(
        include_str!("../../../../tools/rhino_oracle/fixtures/brep_join_edges.json"),
        include_str!("../../../../tools/rhino_oracle/observations/brep_join_edges.json"),
        28,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        close(&a.value, &b["value"], &a.id);
        assert_preserved_surfaces_and_trims(&a.value, &a.id);
        assert_preserved_surfaces_and_trims(&b["value"], &a.id);
    }
}

fn assert_preserved_surfaces_and_trims(value: &Value, id: &str) {
    for field in ["surfaces", "trim_curves"] {
        let original = value["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g[field].as_array().unwrap().clone())
            .collect::<Vec<_>>();
        let output = value["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| g[field].as_array().unwrap().clone())
            .collect::<Vec<_>>();
        assert_eq!(original, output, "{id}/{field}");
    }
}

#[test]
fn low_level_orientation_and_gap_policies_retain_explicit_rhino_differences() {
    let (actual, expected) = responses(
        include_str!("../../../../tools/rhino_oracle/fixtures/brep_join_policies.json"),
        include_str!("../../../../tools/rhino_oracle/observations/brep_join_policies.json"),
        8,
    );
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        close(&a.value["inputs"], &b["value"]["inputs"], &a.id);
        assert_preserved_surfaces_and_trims(&a.value, &a.id);
        assert_preserved_surfaces_and_trims(&b["value"], &a.id);
        let native = &a.value["outputs"][0];
        let rhino = &b["value"]["outputs"][0];
        if a.id.starts_with("faces-") {
            assert_eq!(a.value["outputs"].as_array().unwrap().len(), 1);
            assert_eq!(b["value"]["outputs"].as_array().unwrap().len(), 1);
            for (n, r) in native["face_reversed"]
                .as_array()
                .unwrap()
                .iter()
                .zip(rhino["face_reversed"].as_array().unwrap())
            {
                assert_eq!(n.as_bool().unwrap(), !r.as_bool().unwrap());
            }
            let zero_volume = a.id.starts_with("faces-00-");
            assert_eq!(
                native["volume"].as_f64().unwrap(),
                if zero_volume { 0. } else { -30. }
            );
            assert_eq!(
                rhino["volume"].as_f64().unwrap(),
                if zero_volume { 0. } else { 30. }
            );
            // Every oriented face is the opposite of one recorded Rhino face.
            // Sorting by oriented incidence changes list order after reversal.
            for face in native["faces"].as_array().unwrap() {
                let mut reversed = face.clone();
                for boundary in reversed[0].as_array_mut().unwrap() {
                    for edge in boundary[1].as_array_mut().unwrap() {
                        edge[1] = json!(!edge[1].as_bool().unwrap());
                    }
                }
                assert!(rhino["faces"].as_array().unwrap().contains(&reversed));
            }
            let (mut n, mut r) = (native.clone(), rhino.clone());
            for field in ["faces", "face_reversed", "volume"] {
                n.as_object_mut().unwrap().remove(field);
                r.as_object_mut().unwrap().remove(field);
            }
            close(&n, &r, &a.id);
        } else {
            assert_eq!(a.value["outputs"].as_array().unwrap().len(), 1);
            assert_eq!(native["vertices"].as_array().unwrap().len(), 6);
            assert_eq!(native["edges"].as_array().unwrap().len(), 7);
            // Every native retained spatial definition is still a source curve.
            let definitions = a.value["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|g| g["edges"].as_array().unwrap())
                .map(|e| &e["curve"]["definition"])
                .collect::<Vec<_>>();
            for edge in native["edges"].as_array().unwrap() {
                assert!(definitions.contains(&&edge["curve"]["definition"]));
            }
            assert_eq!(native["vertices"][1][1], json!(3.));
            if a.id == "gap-0.0005" {
                assert_eq!(b["value"]["outputs"].as_array().unwrap().len(), 1);
                assert_eq!(
                    rhino["vertices"][1][1].as_f64().unwrap(),
                    3_f64.midpoint(3.0005)
                );
                assert_eq!(native["edge_tolerances"][0].as_f64().unwrap(), 0.);
                assert!(rhino["edge_tolerances"][0].as_f64().unwrap() > 0.00024);
                assert!(native["edge_tolerances"][3].as_f64().unwrap() > 0.0005);
            } else {
                assert_eq!(a.id, "gap-0.001");
                let outputs = b["value"]["outputs"].as_array().unwrap();
                assert_eq!(outputs.len(), 2);
                // The API failed to join at this threshold, but rebuilt one
                // coincident endpoint in both returned sheets. Inputs stayed intact.
                let midpoint = json!([0., 3_f64.midpoint(3.001), 5.]);
                for output in outputs {
                    assert_eq!(output["vertices"].as_array().unwrap().len(), 4);
                    assert_eq!(output["edges"].as_array().unwrap().len(), 4);
                    assert!(output["vertices"].as_array().unwrap().contains(&midpoint));
                }
            }
        }
    }
}

#[test]
fn large_surface_parameter_origin_retains_rhino_area_residual_and_analytic_witness() {
    let (actual, expected) = responses(
        include_str!("../../../../tools/rhino_oracle/fixtures/brep_join_parameter_origin.json"),
        include_str!("../../../../tools/rhino_oracle/observations/brep_join_parameter_origin.json"),
        1,
    );
    let mut native = actual.results[0].value.clone();
    let mut rhino = expected["results"][0]["value"].clone();
    for v in [&native, &rhino] {
        assert_preserved_surfaces_and_trims(v, "shifted-origin");
    }
    // 70-digit independent integration of the rational profile's speed * 3.
    // C(t) = (t+2t², 2t(1-t), 0)/(1-t+t²), t in [0,1].
    let analytic = 10.221_833_812_673_662;
    for (collection, object, face) in [("inputs", 1, 0), ("outputs", 0, 1)] {
        let a = native[collection][object]["faces"][face][1]
            .take()
            .as_f64()
            .unwrap();
        let b = rhino[collection][object]["faces"][face][1]
            .take()
            .as_f64()
            .unwrap();
        assert!((a - analytic).abs() < 2e-14);
        assert!((b - analytic).abs() > 3e-9 && (b - analytic).abs() < 4e-9);
    }
    close(&native, &rhino, "everything-except-measured-area");
}
