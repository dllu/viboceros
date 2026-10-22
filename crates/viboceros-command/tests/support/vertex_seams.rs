//! Immutable native vertex seam recipes shared by command and getter replay.
use serde_json::Value;
use viboceros_command::vertex_seams::{VertexSeamMode, VertexSeamTarget};
use viboceros_document::ObjectId;
pub fn captures() -> Vec<(&'static str, Value)> {
    let mut rows = Vec::new();
    for (family, capture) in [
        (
            "component_seams_subd",
            include_str!("../../../../tools/rhino_oracle/observations/component_seams_subd.json"),
        ),
        (
            "component_seams_subd_followup",
            include_str!(
                "../../../../tools/rhino_oracle/observations/component_seams_subd_followup.json"
            ),
        ),
    ] {
        let value: Value = serde_json::from_str(capture).unwrap();
        rows.extend(
            value["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| {
                    matches!(
                        r["value"]["input"]["command"].as_str(),
                        Some("WeldVertices" | "UnweldVertex")
                    )
                })
                .cloned()
                .map(|row| (family, row)),
        );
    }
    rows
}
pub fn targets(ids: &[ObjectId], value: &Value) -> Vec<VertexSeamTarget> {
    value["input"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .flat_map(|(s, source)| {
            let object = ids[s];
            source["selected_vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |index| VertexSeamTarget {
                    object,
                    index: index.as_u64().unwrap() as usize,
                })
        })
        .collect()
}
pub fn mode(value: &Value) -> VertexSeamMode {
    if value["input"]["command"] == "WeldVertices" {
        VertexSeamMode::Weld
    } else {
        VertexSeamMode::Unweld {
            modify_normals: value["input"]["modify_normals"].as_bool().unwrap(),
        }
    }
}
