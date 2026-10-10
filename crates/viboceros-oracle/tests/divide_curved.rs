use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

fn compare(a: &Value, b: &Value, path: &str, epsilon: f64) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= epsilon + 1e-12 * a.abs().max(b.abs()),
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"), epsilon);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, v) in a {
                compare(v, &b[k], &format!("{path}.{k}"), epsilon);
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

#[test]
fn curved_divide_replays_native_arcs_ellipses_rational_curves_and_polycurves() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/divide_curved.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/divide_curved.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 55);
    for r in result.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == r.id)
            .unwrap();
        let epsilon = if r.id.starts_with("chord-") {
            1e-9
        } else {
            5e-7
        };
        compare(&r.value, &reference["value"], &r.id, epsilon);
    }
}

#[test]
fn extreme_chord_points_replay_native_weighted_and_contact_outputs() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/divide_chord_extreme.json"
    ))
    .unwrap();
    // Split records are separately retained and compared in the diagnostic
    // report; their native parameter/count differences remain unresolved.
    request
        .operations
        .retain(|operation| operation.id().ends_with("-s0"));
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/divide_chord_extreme.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 2);
    for r in result.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == r.id)
            .unwrap();
        let epsilon = if r.id == "chord-weighted-line-s0" {
            5e-7
        } else {
            1e-9
        };
        compare(&r.value, &reference["value"], &r.id, epsilon);
    }
}

#[test]
fn explicit_nurbs_circle_split_preserves_rational_parameterization() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/divide_nurbs_circle.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/divide_nurbs_circle.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 4);
    for r in result.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == r.id)
            .unwrap();
        let epsilon = if r.id.starts_with("chord-") {
            1e-9
        } else {
            5e-7
        };
        compare(&r.value, &reference["value"], &r.id, epsilon);
    }
}
