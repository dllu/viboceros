//! Independent native recipes shared by API and ordinary GUI command replays.
use super::diagnostic_comparison;
use serde_json::{Value, json};
use std::path::Path;

pub fn cases() -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let mut cases = vec![];
    for (family, count) in [
        ("curve_path_extrusion", 168),
        ("curve_path_continuity", 104),
    ] {
        let request: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let ops = request["operations"].as_array().unwrap();
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
    let mut projected = json!({"succeeded":native["succeeded"],"after":native["after"],"undo":native["undo"],"redo":native["redo"]});
    for phase in ["after", "undo", "redo"] {
        if let Some(rows) = projected[phase].as_array_mut() {
            for row in rows {
                row.as_object_mut().unwrap().remove("selected");
            }
        }
    }
    diagnostic_comparison::compare(actual, &projected)
}
