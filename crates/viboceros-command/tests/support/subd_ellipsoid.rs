use super::{box_native, subd_native};
pub use box_native::{Scene, macro_actions as actions, plane};
use serde_json::Value;
pub fn rows() -> Vec<Value> {
    let mut all = vec![];
    for (data, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/observations/subd_ellipsoid.json"),
            50,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/subd_ellipsoid_details.json"),
            14,
        ),
    ] {
        let data: Value = serde_json::from_str(data).unwrap();
        let rows = data["results"].as_array().unwrap();
        assert_eq!(rows.len(), count);
        for (i, row) in rows.iter().enumerate() {
            let mut row = row.clone();
            row["fresh"] = Value::Bool(i == 0);
            all.push(row);
        }
    }
    all
}
#[allow(dead_code)]
pub fn getter_only(row: &Value) -> bool {
    row["id"] == "subd_ellipsoid_retry_second"
}
pub fn setup(value: &Value) -> Scene {
    box_native::setup_on_layer(value, "SubDEllipsoid output")
}
#[allow(dead_code)]
pub fn command(value: &Value, source: Option<viboceros_document::ObjectId>) -> String {
    format!("SubDEllipsoid {}", actions(value, source).join(" "))
}
pub fn check(scene: &Scene, value: &Value, phase: &str) -> Value {
    let reduced = subd_native::facets::without_surface(value, phase);
    let mut check = box_native::check(scene, &reduced, phase);
    let targets = value[phase]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o.get("subd"));
    let actual = scene.document.objects().filter_map(|o| match o.geometry() {
        viboceros_document::Geometry::SubD(s) => Some(s),
        _ => None,
    });
    for ((actual, target), entry) in actual
        .zip(targets)
        .zip(check["objects"].as_array_mut().unwrap())
    {
        subd_native::facets::certify_common_triangulation(actual, target, entry);
    }
    check
}
