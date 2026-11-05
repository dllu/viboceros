use serde_json::Value;
pub(super) fn compare(a: &Value, b: &Value, path: &str, epsilon: f64) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < epsilon,
                "{path}: {a} vs {b}; epsilon {epsilon}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"), epsilon);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, a) in a {
                compare(a, &b[k], &format!("{path}/{k}"), epsilon);
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
