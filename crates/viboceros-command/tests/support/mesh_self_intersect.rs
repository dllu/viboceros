//! Owned native scenes for independent self-intersections and shared options.
use super::frozen_scene;
use serde_json::Value;

fn root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
}
pub fn rows() -> Vec<Value> {
    let mut rows = Vec::new();
    for family in ["discovery", "controls"] {
        let path = root().join(format!(
            "tools/rhino_oracle/observations/mesh_self_intersect_{family}.json"
        ));
        let data: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for mut row in data["results"].as_array().unwrap().iter().cloned() {
            row["family"] = family.into();
            rows.push(row);
        }
    }
    rows
}
pub fn setup(row: &Value) -> frozen_scene::Scene {
    let path = root().join(format!(
        "tools/rhino_oracle/fixtures/mesh_self_intersect_{}/{}.3dm",
        row["family"].as_str().unwrap(),
        row["id"].as_str().unwrap()
    ));
    frozen_scene::setup(&path, &row["value"], "Boolean output")
}
pub fn name(row: &Value) -> &str {
    row["value"]["input"]["command_name"]
        .as_str()
        .unwrap_or("MeshSelfIntersect")
}
pub fn command(row: &Value) -> String {
    format!(
        "{} {}",
        name(row),
        row["value"]["input"]["arguments"]
            .as_str()
            .unwrap_or("")
            .replace('_', "")
    )
}
pub fn menu(row: &Value) -> bool {
    row["id"].as_str().unwrap().ends_with("_menu")
}

pub const GAPS: [&str; 2] = [
    "mesh_self_intersect_controls_boolean_document",
    "mesh_self_intersect_controls_identical_boxes",
];
