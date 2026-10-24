//! Frozen scenes shared by command and application MeshIntersect replays.
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
    for family in [
        "discovery",
        "controls",
        "modes",
        "coverage",
        "topology",
        "coincident",
        "mixed",
    ] {
        let path = root().join(format!(
            "tools/rhino_oracle/observations/mesh_intersect_{family}.json"
        ));
        let data: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for mut row in data["results"].as_array().unwrap().iter().cloned() {
            if row["value"]["history"]
                .as_str()
                .unwrap()
                .contains("Unknown command:")
            {
                continue;
            }
            row["family"] = family.into();
            rows.push(row);
        }
    }
    rows
}
pub fn setup(row: &Value) -> frozen_scene::Scene {
    let path = root().join(format!(
        "tools/rhino_oracle/fixtures/mesh_intersect_{}/{}.3dm",
        row["family"].as_str().unwrap(),
        row["id"].as_str().unwrap()
    ));
    frozen_scene::setup(&path, &row["value"], "Boolean output")
}
pub fn name(row: &Value) -> &str {
    row["value"]["input"]["command_name"]
        .as_str()
        .unwrap_or("MeshIntersect")
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
pub const GAPS: [&str; 17] = [
    "mesh_intersect_coincident_base",
    "mesh_intersect_coincident_drop",
    "mesh_intersect_coincident_face_order",
    "mesh_intersect_coincident_one_face_order",
    "mesh_intersect_coincident_one_vertex_order",
    "mesh_intersect_coincident_one_winding",
    "mesh_intersect_coincident_rotated",
    "mesh_intersect_coincident_scaled",
    "mesh_intersect_coincident_translated",
    "mesh_intersect_coincident_triangles",
    "mesh_intersect_coincident_vertex_order",
    "mesh_intersect_coincident_winding",
    "mesh_intersect_identical_post",
    "mesh_intersect_identical_pre",
    "mesh_intersect_mixed_corner",
    "mesh_intersect_mixed_curtain",
    "mesh_intersect_modes_mesh_identical",
];
