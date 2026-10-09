use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};
fn compare(a: &Value, b: &Value, path: &str, numbers: &mut usize) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            *numbers += 1;
            let a = a.as_f64().unwrap();
            let b = b.as_f64().unwrap();
            assert!(
                (a - b).abs() <= 1e-9 + 1e-12 * b.abs(),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"), numbers);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, v) in a {
                compare(v, &b[k], &format!("{path}.{k}"), numbers);
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn contour_command_replays_native_grid_range_mesh_groups_and_properties_with_explicit_domain_diagnostics()
 {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/contour_command.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/contour_command.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 21);
    let mut numbers = 0;
    let mut matched = 0;
    let mut diagnostics = 0;
    for result in actual.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == result.id)
            .unwrap();
        if matches!(
            result.id.as_str(),
            "surface-x"
                | "surface-properties-group"
                | "saddle-cuts"
                | "translated-plane"
                | "surface-y"
        ) {
            let mut actual = result.value.clone();
            let mut expected = reference["value"].clone();
            let a = actual["outputs"].as_array_mut().unwrap();
            let b = expected["outputs"].as_array_mut().unwrap();
            assert_eq!(a.len(), b.len());
            assert!(
                a.iter()
                    .zip(b.iter())
                    .any(|(a, b)| a["domain"] != b["domain"])
            );
            for (a, b) in a.iter_mut().zip(b.iter_mut()) {
                a.as_object_mut().unwrap().remove("domain");
                b.as_object_mut().unwrap().remove("domain");
            }
            // Geometry/metadata checks are evidence of those fields only.
            // These five recipes remain failed strict compatibility cases.
            compare(&actual, &expected, &result.id, &mut numbers);
            diagnostics += 1;
        } else {
            compare(&result.value, &reference["value"], &result.id, &mut numbers);
            matched += 1;
        }
    }
    assert_eq!(matched, 16);
    assert_eq!(diagnostics, 5);
    assert!(numbers > 1000);
}
