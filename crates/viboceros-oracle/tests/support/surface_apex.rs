//! Independent licensed command measurements shared by reducer and GUI tests.
use serde_json::Value;
use std::path::Path;
use viboceros_document::{Document, Geometry, ObjectId};

pub fn cases() -> Vec<(Value, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
        .join("tools/rhino_oracle");
    let request: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/surface_apex_command.json")).unwrap(),
    )
    .unwrap();
    let native: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/surface_apex_command_tolerance.json"))
            .unwrap(),
    )
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    let rows = native["results"].as_array().unwrap();
    assert_eq!(operations.len(), 72);
    assert_eq!(rows.len(), operations.len());
    operations
        .iter()
        .zip(rows)
        .map(|(op, row)| {
            assert_eq!(op["id"], row["id"]);
            (op.clone(), row["value"].clone())
        })
        .collect()
}

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

pub fn measure(doc: &Document, source: ObjectId, original: &Geometry, native: &Value) -> Value {
    let value = viboceros_oracle::surface_apex_extrusion::snapshot(doc, source, original).unwrap();
    let error = compare(&value, native);
    serde_json::json!({"value":value,"maximum_error":error})
}
