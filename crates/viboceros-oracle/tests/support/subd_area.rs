use serde_json::Value;
pub fn area(native: &Value) -> f64 {
    native["history"]
        .as_str()
        .unwrap()
        .lines()
        .find_map(|s| {
            s.strip_prefix("Area = ")
                .or_else(|| s.strip_prefix("Cumulative Area = "))
                .and_then(|s| s.split_whitespace().next())
                .and_then(|s| s.parse().ok())
        })
        .expect("native command area")
}
/// Returns the original strict native-coordinate verdict. Mixed controls also
/// check the separately recorded tight SubD mass plus analytic circle area.
pub fn state(actual: &Value, native: &Value, mixed: Option<&Value>) -> bool {
    for key in ["selected", "objects"] {
        assert_eq!(actual[key], native[key], "{key}");
    }
    let a = actual["points"].as_array().unwrap();
    let b = native["points"].as_array().unwrap();
    assert_eq!(a.len(), b.len());
    let mut strict = true;
    for (a, b) in a.iter().zip(b) {
        let error = (0..3)
            .map(|i| (a[i].as_f64().unwrap() - b[i].as_f64().unwrap()).abs())
            .fold(0., f64::max);
        strict &= error < 1e-8;
        if let Some(diagnostics) = mixed {
            let mass = diagnostics
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["mode"] == 3)
                .unwrap();
            let area = mass["area"].as_f64().unwrap();
            let circle = 4. * std::f64::consts::PI;
            for (i, center) in [20., 0., 0.].into_iter().enumerate() {
                let reference = (area * mass["centroid"][i].as_f64().unwrap() + circle * center)
                    / (area + circle);
                assert!(
                    (a[i].as_f64().unwrap() - reference).abs() < 1e-8,
                    "tight mixed centroid {a}, reference coordinate {reference}"
                );
            }
            assert!(error < 1e-6, "native default residual {error}");
        } else {
            assert!(
                strict,
                "native centroid error {error}: actual {a}, native {b}"
            );
        }
    }
    strict
}
