pub const NEW_FAMILIES: [&str; 3] = [
    "blend_curve_surface_high_discovery",
    "blend_curve_surface_high_holdouts",
    "blend_curve_surface_high_planarity",
];
pub const FAMILIES: [&str; 4] = [
    "blend_curve_edge_discovery",
    "blend_curve_surface_high_discovery",
    "blend_curve_surface_high_holdouts",
    "blend_curve_surface_high_planarity",
];
pub fn included(family: &str, name: &str) -> bool {
    family != "blend_curve_edge_discovery"
        || matches!(name, "surface_curved_g3" | "surface_curved_g4")
}

/// The three 2000-unit handle cases preserve their observed domain mismatch.
/// Independent expected-data integration checks their actual geometric lengths.
pub fn independent_length(name: &str) -> Option<f64> {
    let audit: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/blend-curve-surface-high-proof/planarity-lengths.json"
    ))
    .unwrap();
    audit[name]["independent_length"].as_f64()
}
