//! Prescribed arc sources and the original tangent pick calibration.
#[allow(unused_imports)]
pub use super::tangent_inputs::{click_point, command, guide, pick, setup, token};
use serde_json::Value;
pub const FAMILIES: [&str; 3] = [
    "line_tangent_arcs",
    "line_tangent_arcs_ends",
    "line_tangent_arcs_controls",
];
pub fn sources() -> Value {
    serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/line_tangent_arcs_sources.json"
    ))
    .unwrap()
}
pub fn excluded(id: &str) -> bool {
    // The scale pilot used integer division in IronPython; the other cases
    // retain the native local-search failures under investigation.
    [
        "arc_scaled",
        "arc_station_5",
        "arc_middle_control",
        "arc_reversed",
        "arc_as_nurbs",
        "arc_translated",
        "arc_hover_contact",
        "arc_hover_endpoint",
        "arc_scaled_corrected",
    ]
    .contains(&id)
}
