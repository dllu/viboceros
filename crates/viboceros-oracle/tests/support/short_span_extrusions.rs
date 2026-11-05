//! Geometric coordinates have an epsilon; microscopic knot intervals do not.
use serde_json::Value;
pub fn exact_basis(actual: &Value, native: &Value, label: &str) {
    for (a, b) in actual
        .as_array()
        .unwrap()
        .iter()
        .zip(native.as_array().unwrap())
    {
        let (a, b, keys) = if a["kind"] == "curve" {
            (&a["curve"], &b["curve"], &["knots"][..])
        } else {
            (&a["surface"], &b["surface"], &["knots_u", "knots_v"][..])
        };
        for key in keys {
            assert_eq!(
                a[key], b[key],
                "{label}/{key}: preserve every representable span"
            );
        }
        for (a, b) in a["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["control_points"].as_array().unwrap())
        {
            assert_eq!(a["weight"], b["weight"], "{label}: rational weight");
        }
    }
}
