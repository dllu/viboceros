use super::*;

#[test]
fn all_alignment_fixtures_match_recorded_rhino_geometry_and_document_state() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/align.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/align.json"
    ))
    .unwrap();
    let mut actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 48);
    let mixed_request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/align_mixed.json"
    ))
    .unwrap();
    let mixed_expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/align_mixed.json"
    ))
    .unwrap();
    let mixed_actual = run_request(&mixed_request).unwrap();
    assert_eq!(mixed_actual.results.len(), 4);
    actual.results.extend(mixed_actual.results);
    let post_request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/align_postselection.json"
    ))
    .unwrap();
    let post_expected: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/align_postselection.json"
    ))
    .unwrap();
    let post_actual = run_request(&post_request).unwrap();
    assert_eq!(post_actual.results.len(), 4);
    actual.results.extend(post_actual.results);
    let expected = expected["results"]
        .as_array()
        .unwrap()
        .iter()
        .chain(mixed_expected["results"].as_array().unwrap())
        .chain(post_expected["results"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual.results.len(), expected.len());
    fn compare(actual: &Value, expected: &Value, path: &str) {
        match (actual, expected) {
            (Value::Number(a), Value::Number(b)) => assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-8,
                "{path}: {a} != {b}"
            ),
            (Value::Array(a), Value::Array(b)) => {
                assert_eq!(a.len(), b.len(), "{path}");
                for (i, (a, b)) in a.iter().zip(b).enumerate() {
                    compare(a, b, &format!("{path}/{i}"));
                }
            }
            (Value::Object(a), Value::Object(b)) => {
                assert_eq!(
                    a.keys().collect::<Vec<_>>(),
                    b.keys().collect::<Vec<_>>(),
                    "{path}"
                );
                for (k, a) in a {
                    compare(a, &b[k], &format!("{path}/{k}"));
                }
            }
            _ => assert_eq!(actual, expected, "{path}"),
        }
    }
    for (a, b) in actual.results.iter().zip(expected) {
        assert_eq!(a.id, b["id"]);
        compare(&a.value, &b["value"], &a.id);
    }
}

#[test]
fn curved_alignment_discrepancies_are_checked_against_analytic_extrema() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/align_diagnostics.json"
    ))
    .unwrap();
    let rhino: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/align_diagnostics.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 3);
    assert_eq!(
        actual.results.len(),
        rhino["results"].as_array().unwrap().len()
    );
    for ((operation, result), reference) in request
        .operations
        .iter()
        .zip(actual.results)
        .zip(rhino["results"].as_array().unwrap())
    {
        let Operation::Align { id, fixture } = operation else {
            panic!("Align")
        };
        assert_eq!(reference["id"], *id);
        // The unnormalized X projection is u+2v+3z. On the square surface,
        // z=4u(1-u)+8v(1-v), so maxima occur at u=v=13/24 and equal 169/16.
        // On the radius-0.8 paraboloid disk, z=u²+v². Its minimum is -5/12
        // at (-1/6,-1/3), and its maximum is 3r²+r*sqrt(5) on the boundary.
        let surface = id.starts_with("surface-");
        let (low, high): (f64, f64) = if surface {
            (0., 169. / 16.)
        } else {
            (-5. / 12., 3. * 0.8_f64.powi(2) + 0.8 * 5_f64.sqrt())
        };
        let right = fixture.mode == "Right";
        // Other source points fix the overall X range [-22,92], and the Y
        // projection (-2u+v) range [-25,12]. The two directions are orthogonal.
        let x = if right {
            (92. - high) / 14.
        } else {
            (35. - low.midpoint(high)) / 14.
        };
        let y = if right {
            0.
        } else {
            (-6.5 - if surface { -0.5 } else { 0. }) / 5.
        };
        let expected = [x - 2. * y, 2. * x + y, 3. * x];
        let geometry = fixture.layout.sources[1]
            .geometry(request.tolerance.geometry().unwrap())
            .unwrap();
        let (_, original) = crate::object_layout::sample(&geometry).unwrap();
        let points = result.value["objects"][1]["points"].as_array().unwrap();
        let reference_points = reference["value"]["objects"][1]["points"]
            .as_array()
            .unwrap();
        assert_eq!(points.len(), original.len());
        assert_eq!(points.len(), reference_points.len());
        let mut measured_discrepancy = 0_f64;
        for ((a, b), r) in points.iter().zip(original).zip(reference_points) {
            for axis in 0..3 {
                let a = a[axis].as_f64().unwrap();
                assert!(
                    (a - b[axis] - expected[axis]).abs() < 1e-8,
                    "{id} axis {axis}"
                );
                measured_discrepancy =
                    measured_discrepancy.max((a - r[axis].as_f64().unwrap()).abs());
            }
        }
        assert!(
            measured_discrepancy > 1e-6,
            "{id}: discrepancy unexpectedly disappeared; review the reference"
        );
    }
}
