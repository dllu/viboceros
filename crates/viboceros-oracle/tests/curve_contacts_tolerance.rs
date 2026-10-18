#![cfg(feature = "native-smlib")]
use serde_json::Value;
use viboceros_oracle::{ProbeRequest, run_request};

fn compare(actual: &Value, expected: &Value, epsilon: f64) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, expected) in b {
                compare(a.get(key).expect("missing SDK field"), expected, epsilon);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (actual, expected) in a.iter().zip(b) {
                compare(actual, expected, epsilon);
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            assert!((a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= epsilon);
        }
        _ => assert_eq!(actual, expected),
    }
}

#[test]
fn complete_intersect_command_values_match_rhino_for_in_tolerance_skew_contacts() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_contacts_tolerance.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_contacts_tolerance.json"
    ))
    .unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), 5);
    for (a, b) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        assert_eq!(a.id, b["id"].as_str().unwrap());
        compare(&a.value, &b["value"], 1e-8);
    }
}
