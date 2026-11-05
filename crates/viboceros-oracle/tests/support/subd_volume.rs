use serde_json::Value;
pub fn value(native: &Value) -> Option<f64> {
    native["history"].as_str().unwrap().lines().find_map(|s| {
        s.strip_prefix("Volume = ")
            .or_else(|| s.strip_prefix("Cumulative Volume = "))
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse().ok())
    })
}
pub fn state(actual: &Value, native: &Value) {
    for key in ["selected", "objects"] {
        assert_eq!(actual[key], native[key], "{key}");
    }
    let a = actual["points"].as_array().unwrap();
    let b = native["points"].as_array().unwrap();
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        for i in 0..3 {
            assert!(
                (a[i].as_f64().unwrap() - b[i].as_f64().unwrap()).abs() < 1e-6,
                "centroid {a} versus native {b}"
            );
        }
    }
}
pub fn definition(actual: &Value, native: &Value) {
    for key in ["faces", "vertex_tags"] {
        assert_eq!(actual[key], native[key], "{key}");
    }
    let a = actual["points"].as_array().unwrap();
    let b = native["points"].as_array().unwrap();
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        for i in 0..3 {
            assert!(
                (a[i].as_f64().unwrap() - b[i].as_f64().unwrap()).abs() < 1e-12,
                "source points"
            );
        }
    }
}
