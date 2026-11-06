//! Fixed inputs are kept separate from immutable native outputs.
use super::native;
use serde_json::Value;
use viboceros_document::Document;
use viboceros_geometry::Tolerance;
pub const FAMILIES: [&str; 4] = [
    "line_construction",
    "line_construction_holdouts",
    "line_construction_precision",
    "line_construction_completion",
];
pub const DIAGNOSTICS: [&str; 14] = [
    "both_initial",
    "cancel_both",
    "vertical_length",
    "vertical_negative",
    "vertical_both",
    "bisector_length",
    "angled_length",
    "native_epsilon_below",
    "bisector_opposite",
    "tiny_1e12",
    "tiny_1e10",
    "tiny_zero_tolerance",
    "tiny_1e9",
    "tiny_repeat_then_end",
];
pub const NATIVE_INVALID: [&str; 3] = [
    "tiny_complete_1e12",
    "tiny_complete_1e10",
    "tiny_complete_zero_tolerance",
];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_construction_sources.json"
    ))
    .unwrap()
}
pub fn setup(v: &Value) -> Document {
    let mut doc = native::scene(v);
    let t = &v["model_tolerance"];
    doc.set_tolerance(
        Tolerance::try_new(
            t["absolute"].as_f64().unwrap(),
            t["relative"].as_f64().unwrap(),
            t["angular"].as_f64().unwrap(),
        )
        .unwrap(),
    );
    doc.clear_undo_records().unwrap();
    doc
}
pub fn token(value: &Value, world: bool) -> String {
    if let Some(p) = value.as_array() {
        format!(
            "{}{}",
            if world { "w" } else { "" },
            p.iter()
                .map(|n| n.as_f64().unwrap().to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    } else if let Some(w) = value.as_str() {
        w.to_owned()
    } else {
        value.to_string()
    }
}
