use super::box_native;
use super::torus_surface as surface;
pub use box_native::{Scene, plane};
use serde_json::Value;
pub fn rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/subd_torus.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 48);
    rows.clone()
}
pub fn setup(value: &Value) -> Scene {
    box_native::setup_on_layer(value, "SubDTorus output")
}
#[allow(dead_code)]
pub fn getter_only(row: &Value) -> bool {
    row["id"] == "subd_torus_default_small_retry"
}
pub fn actions(value: &Value, source: Option<viboceros_document::ObjectId>) -> Vec<String> {
    let mut words = box_native::actions(value, source);
    // Replay the actual numeric tokens emitted by IronPython, which may round
    // JSON recipe values. Boundary admission depends on these exact inputs.
    let macro_words = value["macro"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>();
    let mut at = 1;
    for (index, action) in value["input"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if action.is_number() {
            words[index] = macro_words[at].into();
        }
        at += if action.is_object() { 2 } else { 1 };
    }
    words
}
#[allow(dead_code)]
pub fn command(value: &Value, source: Option<viboceros_document::ObjectId>) -> String {
    format!("SubDTorus {}", actions(value, source).join(" "))
}
pub fn check(scene: &Scene, value: &Value, phase: &str) -> Value {
    let mut check = box_native::check(scene, &surface::without_surface(value, phase), phase);
    let targets = value[phase]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o.get("subd"))
        .collect::<Vec<_>>();
    let actual = scene
        .document
        .objects()
        .filter_map(|o| match o.geometry() {
            viboceros_document::Geometry::SubD(s) => Some(s),
            _ => None,
        })
        .collect::<Vec<_>>();
    for ((actual, native), entry) in actual
        .into_iter()
        .zip(targets)
        .zip(check["objects"].as_array_mut().unwrap())
    {
        surface::certify(actual, native, entry);
    }
    check
}
