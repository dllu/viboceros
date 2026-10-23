//! Exact command sources and preference transitions witnessed by native prompts.
use serde_json::{Value, json};
use viboceros_command::MeshRebuildOptions;
pub const EXCLUDED: &str = "rebuild_mesh_discovery_preselected_flags";

pub fn groups() -> Vec<(&'static str, Vec<Value>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    [
        ("rebuild_mesh_discovery", 5),
        ("rebuild_mesh_command", 21),
        ("rebuild_mesh_holdouts", 6),
    ]
    .into_iter()
    .map(|(family, count)| {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let rows = data["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["id"] != EXCLUDED)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), count);
        (family, rows)
    })
    .collect()
}

pub fn arguments(value: &Value) -> Vec<&str> {
    value["input"]["command_arguments"]
        .as_str()
        .unwrap_or("")
        .split_whitespace()
        .filter(|s| !matches!(s.trim_start_matches('_'), "All" | "Enter"))
        .collect()
}

pub fn staged(value: &Value, mut before: MeshRebuildOptions) -> MeshRebuildOptions {
    for line in value["history"].as_str().unwrap().lines() {
        let Some((_, rest)) = line.split_once("PreserveTextureCoordinates=") else {
            continue;
        };
        let Some((_, colors)) = rest.split_once("PreserveVertexColors=") else {
            panic!();
        };
        before.preserve_texture_coordinates = rest
            .split_whitespace()
            .next()
            .unwrap()
            .trim_end_matches([')', ':'])
            == "Yes";
        before.preserve_vertex_colors = colors
            .split_whitespace()
            .next()
            .unwrap()
            .trim_end_matches([')', ':'])
            == "Yes";
    }
    before
}
pub fn report(row: &Value, options: MeshRebuildOptions, maximum: f64) -> Value {
    json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"normals":true,"ngons":true,"identity":true,"metadata":true,"selection":true,"object_order":true,"history":true,"staged":true,"accepted":row["value"]["success"],"preferences":{"preserve_texture_coordinates":options.preserve_texture_coordinates,"preserve_vertex_colors":options.preserve_vertex_colors},"maximum_error":maximum})
}
