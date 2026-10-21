use super::cylinder_native;
use super::subd_native::facets as surface;
pub use cylinder_native::{Scene, actions, point, token};
use serde_json::Value;
pub fn rows() -> Vec<Value> {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/subd_torus_references.json"
    ))
    .unwrap();
    let rows = data["results"].as_array().unwrap();
    assert_eq!(rows.len(), 29);
    rows.clone()
}
pub fn setup(value: &Value) -> Scene {
    cylinder_native::setup_on_layer(value, "SubDTorus output")
}
#[allow(dead_code)]
pub fn command(scene: &mut Scene, value: &Value) -> (String, bool) {
    cylinder_native::command_for("SubDTorus", scene, value)
}
pub fn check(scene: &Scene, value: &Value, phase: &str) -> Value {
    let mut check = cylinder_native::check(scene, &surface::without_surface(value, phase), phase);
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
