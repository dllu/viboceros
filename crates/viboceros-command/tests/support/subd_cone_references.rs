use super::cylinder_native;
pub use cylinder_native::{Scene, actions, check, point, token};
use serde_json::Value;
pub fn rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/subd_cone_references.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 24);
    rows.iter()
        .filter(|r| r["id"] != "cone_tangent_base_angle")
        .cloned()
        .collect()
}
pub fn setup(value: &Value) -> Scene {
    cylinder_native::setup_on_layer(value, "SubDCone output")
}
#[allow(dead_code)]
pub fn command(scene: &mut Scene, value: &Value) -> (String, bool) {
    cylinder_native::command_for("SubDCone", scene, value)
}
