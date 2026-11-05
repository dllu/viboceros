use serde_json::Value;

pub fn compare(a: &Value, b: &Value, path: &str) {
    // Native factories can choose either endpoint order for an undirected
    // SubD edge. Face cycles retain the independently checked orientation.
    if path.ends_with("/edges") {
        let canonical = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let mut pair = pair
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| i.as_u64().unwrap())
                        .collect::<Vec<_>>();
                    pair.sort_unstable();
                    pair
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(canonical(a), canonical(b), "{path}");
        return;
    }
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if a.is_f64() || b.is_f64() => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() < 1e-12,
            "{path}: {a} vs {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
