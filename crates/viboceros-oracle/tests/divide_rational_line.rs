use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

fn matches(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            (a - b).abs() <= 5e-7 + 1e-12 * a.abs().max(b.abs())
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| matches(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.keys().eq(b.keys()) && a.iter().all(|(k, v)| matches(v, &b[k]))
        }
        _ => a == b,
    }
}

#[test]
fn moderate_weighted_lines_corners_and_polycurves_replay_native_division() {
    let mut request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/divide_rational_line.json"
    ))
    .unwrap();
    // Extreme-weight captures are retained as diagnostics, including native
    // zero-output cases; they do not qualify moderate-weight compatibility.
    request.operations.retain(|o| o.id().contains("moderate"));
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/divide_rational_line.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 16);
    for r in result.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == r.id)
            .unwrap();
        assert!(matches(&r.value, &reference["value"]), "{}", r.id);
    }
}
