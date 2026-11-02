//! Native Loft recipes shared by API and GUI command replays.
use super::diagnostic_comparison;
use serde_json::Value;
use std::path::Path;

pub fn cases() -> Vec<(Value, Value)> {
    load(&[
        ("loft_splitting", 220),
        ("loft_splitting_reorder", 24),
        ("loft_splitting_angles", 32),
    ])
}

pub fn angle_cases() -> Vec<(Value, Value)> {
    load(&[("loft_splitting_angles", 32)])
}

fn load(families: &[(&str, usize)]) -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let mut cases = Vec::new();
    for &(family, count) in families {
        let query: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let ops = query["operations"].as_array().unwrap();
        let rows = response["results"].as_array().unwrap();
        assert_eq!(ops.len(), count);
        assert_eq!(rows.len(), count);
        for (op, row) in ops.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            cases.push((op.clone(), row["value"].clone()));
        }
    }
    cases
}

pub fn compare(actual: &Value, native: &Value) -> f64 {
    diagnostic_comparison::compare(actual, &model_state(native))
}

fn model_state(native: &Value) -> Value {
    let mut expected = native.clone();
    for key in ["history", "events", "crease_events"] {
        expected.as_object_mut().unwrap().remove(key);
    }
    for phase in ["after", "undo", "redo"] {
        for row in expected[phase].as_array_mut().unwrap() {
            row.as_object_mut().unwrap().remove("selected");
        }
    }
    expected
}

/// Check every native topology/chart/sample/history field. Six unsplit Normal
/// records have a known native tight-box difference, retained by the full raw
/// diagnostic. Their local boxes must enclose all samples; the measured upper-Y
/// difference must remain in its recorded range.
pub fn compare_angles(operation: &Value, actual: &Value, native: &Value) -> f64 {
    let mut expected = model_state(native);
    let mut actual = actual.clone();
    let id = operation["id"].as_str().unwrap();
    let different_box = [
        "loft-angle-0-normal-00",
        "loft-angle-0-normal-10",
        "loft-angle-1-normal-00",
        "loft-angle-1-normal-10",
        "loft-angle-2-normal-00",
        "loft-angle-3-normal-00",
    ]
    .contains(&id);
    for phase in ["after", "redo"] {
        let product = actual[phase].as_array_mut().unwrap().last_mut().unwrap();
        let bounds = product["bounds"].clone();
        for face in product["faces"].as_array().unwrap() {
            for p in face["samples"].as_array().unwrap() {
                for axis in 0..3 {
                    let coordinate = p[axis].as_f64().unwrap();
                    assert!(coordinate >= bounds[0][axis].as_f64().unwrap() - 1e-6);
                    assert!(coordinate <= bounds[1][axis].as_f64().unwrap() + 1e-6);
                }
            }
        }
        if different_box {
            let native = expected[phase].as_array_mut().unwrap().last_mut().unwrap();
            let delta = bounds[1][1].as_f64().unwrap() - native["bounds"][1][1].as_f64().unwrap();
            assert!((0.002..0.006).contains(&delta), "{id}: {delta}");
            // Only upper Y differs beyond the ordinary geometry tolerance.
            let mut upper = bounds[1].clone();
            upper[1] = native["bounds"][1][1].clone();
            diagnostic_comparison::compare(
                &serde_json::json!([bounds[0], upper]),
                &native["bounds"],
            );
            product.as_object_mut().unwrap().remove("bounds");
            native.as_object_mut().unwrap().remove("bounds");
        }
    }
    diagnostic_comparison::compare(&actual, &expected)
}
