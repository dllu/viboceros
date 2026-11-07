pub const FAMILIES: [&str; 5] = [
    "blend_curve_alt_pilot",
    "blend_curve_alt_verification",
    "blend_curve_alt_followups",
    "blend_curve_alt_picking",
    "blend_curve_alt_selection",
];
pub fn included(_: &str, name: &str) -> bool {
    name != "plain_alt_far_surface"
}
