//! Fixed captures shared by geometry, command and frontend seam replays.
use serde_json::Value;
use viboceros_command::component_seams::EdgeSeamMode;
pub fn captures() -> Vec<(&'static str, Value)> {
    let mut rows = Vec::new();
    for (family, capture) in [
        (
            "component_seams_subd",
            include_str!("../../../../tools/rhino_oracle/observations/component_seams_subd.json"),
        ),
        (
            "component_seams_subd_edges",
            include_str!(
                "../../../../tools/rhino_oracle/observations/component_seams_subd_edges.json"
            ),
        ),
        (
            "component_seams_subd_edge_holdouts",
            include_str!(
                "../../../../tools/rhino_oracle/observations/component_seams_subd_edge_holdouts.json"
            ),
        ),
        (
            "component_seams_subd_edge_contacts",
            include_str!(
                "../../../../tools/rhino_oracle/observations/component_seams_subd_edge_contacts.json"
            ),
        ),
        (
            "component_seams_subd_edge_tolerances_v2",
            include_str!(
                "../../../../tools/rhino_oracle/observations/component_seams_subd_edge_tolerances_v2.json"
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
                        Some("WeldEdge" | "UnweldEdge")
                    )
                })
                .cloned()
                .map(|row| (family, row)),
        );
    }
    rows
}
pub fn mode(value: &Value) -> EdgeSeamMode {
    if value["input"]["command"] == "WeldEdge" {
        EdgeSeamMode::Weld
    } else {
        EdgeSeamMode::Unweld {
            modify_normals: value["input"]["modify_normals"].as_bool().unwrap_or(true),
        }
    }
}
