use serde_json::Value;

pub fn compare(actual: &Value, native: &Value, path: &str) {
    match (actual, native) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (key, value) in b {
                compare(&a[key], value, &format!("{path}/{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-12,
                "{path}: {a} vs {b}"
            );
        }
        _ => assert_eq!(actual, native, "{path}"),
    }
}
