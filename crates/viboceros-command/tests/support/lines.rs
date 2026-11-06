//! Prescribed adjoining-line inputs and shared document setup.
use super::native;
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry};
use viboceros_geometry::Tolerance;
pub const FAMILIES: [&str; 6] = [
    "lines_command",
    "lines_holdouts",
    "lines_precision",
    "lines_finish",
    "lines_boundaries",
    "lines_macros",
];
pub const DIAGNOSTICS: [&str; 3] = ["explicit_return", "enter_anchor", "enter_empty"];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/lines_sources.json"
    ))
    .unwrap()
}
pub fn setup(v: &Value, source: &Value) -> Document {
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
    if source["pending_redo"] == true {
        let r = CommandRegistry::with_builtins();
        r.execute(&mut doc, "Point 90,91,92").unwrap();
        r.execute(&mut doc, "Undo").unwrap();
    }
    doc
}

pub fn check_types(doc: &Document, expected: &Value) {
    for (object, target) in doc.objects().zip(expected.as_array().unwrap()) {
        assert!(match target["native_type"].as_str().unwrap() {
            "Point" => matches!(object.geometry(), Geometry::Point(_)),
            "LineCurve" => matches!(object.geometry(), Geometry::Line(_)),
            other => panic!("unexpected Lines geometry type: {other}"),
        });
    }
}
