use serde_json::{Value, json};
use std::path::Path;

pub fn cases() -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let mut cases = Vec::new();
    for (family, count) in [
        ("direction_analysis_initial", 12),
        ("direction_analysis_controls_v2", 30),
        ("direction_analysis_membership_v3", 2),
        ("direction_analysis_foreign", 4),
        ("direction_analysis_multi", 3),
    ] {
        let query: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native: Value = serde_json::from_reader(flate2::read::GzDecoder::new(
            std::fs::File::open(root.join(format!("observations/{family}.json.gz"))).unwrap(),
        ))
        .unwrap();
        let ops = query["operations"].as_array().unwrap();
        let rows = native["results"].as_array().unwrap();
        assert_eq!(ops.len(), count);
        assert_eq!(rows.len(), count);
        for (op, row) in ops.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            assert!(row.get("error").is_none());
            let v = &row["value"];
            let mut settings = vec![Value::Null];
            for (step, trace) in op["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(v["traces"].as_array().unwrap())
            {
                assert_eq!(trace["command"], *step);
                assert_eq!(trace["success"], true);
                let controls = trace["controls"].as_array().unwrap();
                if controls.is_empty() {
                    settings.push(Value::Null);
                } else {
                    let mut flags = serde_json::Map::new();
                    for label in ["Direction", "U", "V"] {
                        let matching = controls
                            .iter()
                            .filter(|r| {
                                r["label"].as_str().unwrap().replace('&', "") == label
                                    && r["kind"] == "Button"
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(matching.len(), 1);
                        let check = matching[0]["check"].as_u64().unwrap();
                        assert!(check <= 1);
                        flags.insert(label.to_lowercase(), json!(check == 1));
                    }
                    settings.push(Value::Object(flags));
                }
            }
            cases.push((
                op.clone(),
                json!({"states":v["states"],"settings":settings}),
            ));
        }
    }
    cases
}

pub fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, value) in b {
                compare(&a[key], value, &format!("{path}.{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}[{i}]"));
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            if b.is_u64() {
                assert_eq!(a, b, "{path}");
            }
            let error = (a.as_f64().unwrap() - b.as_f64().unwrap()).abs();
            assert!(error <= 1e-9, "{path}: error {error}, {a} != {b}");
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
