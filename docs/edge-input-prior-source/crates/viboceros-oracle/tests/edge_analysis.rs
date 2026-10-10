use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

fn compare(actual: &Value, native: &Value, path: &str, numbers: &mut usize) {
    match (actual, native) {
        (Value::Number(a), Value::Number(b)) => {
            *numbers += 1;
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                a.is_finite()
                    && b.is_finite()
                    && (a - b).abs() <= 1e-9 + 1e-12 * a.abs().max(b.abs()),
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
            for (key, value) in a {
                compare(value, &b[key], &format!("{path}.{key}"), numbers);
            }
        }
        _ => assert_eq!(actual, native, "{path}"),
    }
}

#[test]
fn edge_analysis_replays_native_order_domains_samples_and_classifications() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/edge_analysis.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/edge_analysis.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 10);
    let mut numbers = 0;
    for result in actual.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == result.id)
            .unwrap();
        compare(&result.value, &reference["value"], &result.id, &mut numbers);
    }
    assert_eq!(numbers, 2268);
}

#[test]
fn edge_mark_replays_native_navigation_duplicates_creation_order_and_history() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/edge_mark_workflow.json"
    ))
    .unwrap();
    let native: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/edge_mark_workflow.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 50);
    let mut numbers = 0;
    for result in actual.results {
        let reference = native["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == result.id)
            .unwrap();
        compare(&result.value, &reference["value"], &result.id, &mut numbers);
    }
    assert!(numbers > 500);
}
