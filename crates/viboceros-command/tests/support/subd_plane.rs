use super::box_native;
pub use box_native::{Scene, check, plane};
use serde_json::Value;
pub fn rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/subd_plane.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 19);
    rows.clone()
}
pub fn setup(value: &Value) -> Scene {
    box_native::setup_on_layer(value, "SubDPlane output")
}
pub fn header(value: &Value) -> String {
    let counts = value["input"]
        .get("counts")
        .map(|c| format!(" XCount={} YCount={}", c[0], c[1]))
        .unwrap_or_default();
    format!("SubDPlane{counts}")
}
pub fn actions(value: &Value, source: Option<viboceros_document::ObjectId>) -> Vec<String> {
    let mut actions = box_native::actions(value, source);
    // The prefix-count capture retains an Enter after command completion.
    if actions.last().is_some_and(|v| v == "Enter") {
        actions.pop();
    }
    actions
}
#[allow(dead_code)]
pub fn command(value: &Value, source: Option<viboceros_document::ObjectId>) -> String {
    format!("{} {}", header(value), actions(value, source).join(" "))
}
