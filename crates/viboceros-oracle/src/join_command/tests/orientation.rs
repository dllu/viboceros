use super::*;

// Zero absolute epsilon: a fixed modeling-scale epsilon could hide an entire
// tiny model or nonzero uncertainty where the reference records exact zero.
fn relative_close(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-12 * a.abs().max(b.abs()),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                relative_close(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                relative_close(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

#[test]
fn raw_join_replay_retains_large_rhino_orientation_discrepancies() {
    let (mut matched, mut gaps, mut inward_rhino) = (0, 0, 0);
    for (input, observation) in [
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/join_orientation.json"),
            include_str!("../../../../../tools/rhino_oracle/observations/join_orientation.json"),
        ),
        (
            include_str!("../../../../../tools/rhino_oracle/fixtures/join_orientation_large.json"),
            include_str!(
                "../../../../../tools/rhino_oracle/observations/join_orientation_large.json"
            ),
        ),
    ] {
        let (actual, expected) = super::surfaces::replay(input, observation, 32);
        let fixture: Value = serde_json::from_str(input).unwrap();
        for (index, (a, b)) in actual
            .results
            .iter()
            .zip(expected["results"].as_array().unwrap())
            .enumerate()
        {
            if a.id.starts_with("scale-360-") && a.id.ends_with("pre-True") {
                let first_reversed = fixture["operations"][index]["sources"][0]["brep"]["reversed"]
                    .as_bool()
                    .unwrap();
                let mut native = a.value.clone();
                let mut rhino = b["value"].clone();
                for (n, r) in native["objects"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .zip(rhino["objects"].as_array_mut().unwrap())
                {
                    if n["source"].is_null() {
                        let (n, r) = (&mut n["brep"], &mut r["brep"]);
                        assert_eq!(n["solid"], true);
                        assert_eq!(r["solid"], true);
                        assert_eq!(
                            n.as_object_mut().unwrap().remove("orientation").unwrap(),
                            "Outward"
                        );
                        assert_eq!(
                            r.as_object_mut().unwrap().remove("orientation").unwrap(),
                            "None"
                        );
                        for (n, r) in n["geometry"]["topology"]["faces"]
                            .as_array_mut()
                            .unwrap()
                            .iter_mut()
                            .zip(r["geometry"]["topology"]["faces"].as_array_mut().unwrap())
                        {
                            assert_eq!(
                                n.as_object_mut().unwrap().remove("reversed").unwrap(),
                                false
                            );
                            assert_eq!(
                                r.as_object_mut().unwrap().remove("reversed").unwrap(),
                                first_reversed
                            );
                        }
                    }
                }
                // Only the separately asserted getter and whole-shell face
                // sense differ. Every other raw definition/document field must match.
                relative_close(&native, &rhino, &a.id);
                gaps += 1;
                inward_rhino += usize::from(first_reversed);
            } else {
                relative_close(&a.value, &b["value"], &a.id);
                matched += 1;
            }
        }
    }
    assert_eq!((matched, gaps, inward_rhino), (48, 16, 8));
}

fn multiply(value: &mut Value, scale: f64) {
    *value = json!(value.as_f64().unwrap() * scale);
}
fn multiply_array(value: &mut Value, scale: f64) {
    for n in value.as_array_mut().unwrap() {
        multiply(n, scale);
    }
}
fn scale_controls(definition: &mut Value, scale: f64) {
    for cp in definition["control_points"].as_array_mut().unwrap() {
        multiply_array(&mut cp["point"], scale);
    }
}

// This is an analytic scale-equivariance check, NOT a Rhino tiny-source
// comparison: OpenNURBS rejects these tiny faces before command execution.
fn scale_spatial_definition(record: &mut Value, scale: f64) {
    let geometry = &mut record["geometry"];
    for vertex in geometry["vertices"].as_array_mut().unwrap() {
        multiply_array(&mut vertex["point"], scale);
        multiply(&mut vertex["tolerance"], scale);
    }
    for edge in geometry["edges"].as_array_mut().unwrap() {
        multiply(&mut edge["tolerance"], scale);
        let curve = &mut edge["curve"]["definition"];
        scale_controls(curve, scale);
        // These box edge parameter intervals equal their physical lengths.
        multiply_array(&mut curve["domain"], scale);
        multiply_array(&mut curve["knots"], scale);
    }
    for face in geometry["faces"].as_array_mut().unwrap() {
        scale_controls(&mut face["definition"], scale);
    }
    // Surface/UV parameterization, weights, topology and face senses remain
    // untouched, including every trim coefficient and native parameter knot.
}

#[test]
fn native_only_tiny_commands_are_exact_power_of_two_images_of_unit_witnesses() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/join_orientation_tiny.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/join_orientation.json"
    ))
    .unwrap();
    assert_eq!(actual.results.len(), 32);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id.replacen("scale--360-", "scale-0-", 1), b["id"]);
        let mut scaled = a.value.clone();
        multiply(&mut scaled["absolute_tolerance"], 2_f64.powi(360));
        for object in scaled["objects"].as_array_mut().unwrap() {
            scale_spatial_definition(&mut object["brep"], 2_f64.powi(360));
        }
        // Binary power-of-two rescaling is exact for these coefficients.
        assert_eq!(scaled, b["value"], "{}", a.id);
    }
}
