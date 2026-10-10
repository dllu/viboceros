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
fn divide_replays_native_count_arc_length_chords_splits_metadata_and_groups() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/divide_command.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/divide_command.json"
    ))
    .unwrap();
    let result = run_request(&request).unwrap();
    assert_eq!(result.results.len(), 30);
    for r in result.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == r.id)
            .unwrap();
        let epsilon = if r.id.starts_with("length-circle-") {
            5e-7
        } else {
            1e-9
        };
        compare(&r.value, &reference["value"], &r.id, epsilon);
    }
}
