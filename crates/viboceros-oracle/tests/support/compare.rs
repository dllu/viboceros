//! Strict diagnostic comparison with tolerant continuous geometry values.
use serde_json::Value;

pub fn compare(actual: &Value, native: &Value) -> f64 {
    match (actual, native) {
        (Value::Number(a), Value::Number(b)) => {
            if b.is_u64() {
                assert_eq!(a, b, "discrete geometry count differs");
            }
            let error = (a.as_f64().unwrap() - b.as_f64().unwrap()).abs();
            assert!(
                error <= 1e-6,
                "geometry error {error}: {actual} != {native}"
            );
            error
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{actual} != {native}");
            a.iter()
                .zip(b)
                .map(|(a, b)| compare(a, b))
                .fold(0., f64::max)
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            a.iter()
                .map(|(key, a)| compare(a, &b[key]))
                .fold(0., f64::max)
        }
        _ => {
            assert_eq!(actual, native);
            0.
        }
    }
}
