//! Complete raw JSON comparison; callers explicitly choose numeric tolerances.
use serde_json::Value;

pub(crate) fn close(a: &Value, b: &Value, path: &str, absolute: f64, relative: f64) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            if a.is_u64() && b.is_u64() {
                assert_eq!(a.as_u64(), b.as_u64(), "{path}");
            } else if a.is_i64() && b.is_i64() {
                assert_eq!(a.as_i64(), b.as_i64(), "{path}");
            } else {
                let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
                assert!(
                    (a - b).abs() <= absolute.max(relative * a.abs().max(b.abs())),
                    "{path}: {a} != {b}"
                );
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                close(a, b, &format!("{path}/{i}"), absolute, relative);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, a) in a {
                close(a, &b[key], &format!("{path}/{key}"), absolute, relative);
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

#[test]
fn raw_comparison_keeps_keys_lengths_and_large_integer_identity() {
    use serde_json::json;
    close(
        &json!([1.0, 0.0]),
        &json!([1. + 1e-11, 1e-12]),
        "close",
        1e-10,
        1e-12,
    );
    for (a, b) in [
        (json!([1]), json!([1, 2])),
        (json!({"a":1}), json!({"b":1})),
        (json!(true), json!(false)),
        (json!(1_u64 << 53), json!((1_u64 << 53) + 1)),
        (json!(1.), json!(1.01)),
    ] {
        assert!(std::panic::catch_unwind(|| close(&a, &b, "different", 1e-10, 1e-12)).is_err());
    }
}
