pub const FAMILIES: [&str; 8] = [
    "blend_curve_control_discovery",
    "blend_curve_control_policy",
    "blend_curve_control_calibration",
    "blend_curve_control_holdouts",
    "blend_curve_control_validation",
    "blend_curve_control_numeric",
    "blend_curve_control_symmetry",
    "blend_curve_alt_selection",
];
pub fn included(family: &str, name: &str) -> bool {
    if family == "blend_curve_alt_selection" {
        return name == "plain_alt_far_surface";
    }
    !matches!(
        name,
        "control_secondary_numeric" | "control_numeric_plane" | "control_numeric_curved"
    )
}
pub fn domain_diagnostic(family: &str, name: &str) -> bool {
    family == "blend_curve_control_validation" && name == "control_validate_rational_g4"
}
