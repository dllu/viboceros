use super::box_native;
pub use box_native::{Scene, actions, check, plane};
use serde_json::Value;
pub fn rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/subd_truncated_cone.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 33);
    rows.clone()
}
pub fn setup(value: &Value) -> Scene {
    box_native::setup_on_layer(value, "SubDTruncatedCone output")
}
#[allow(dead_code)]
pub fn command(value: &Value, source: Option<viboceros_document::ObjectId>) -> String {
    format!("SubDTruncatedCone {}", actions(value, source).join(" "))
}
