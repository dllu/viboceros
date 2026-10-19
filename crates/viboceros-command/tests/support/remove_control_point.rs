//! Retained native getter differences; no geometry tolerance is relaxed.
//! Nine native no-ops and one native end-row removal. Workflow checks remain strict.
pub const KNOWN_GETTER_DIFFERENCES: &[&str] = &[
    "surface_V",
    "surface_toggle",
    "trimmed_rectangle_V",
    "trimmed_rectangle_toggle",
    "trimmed_hole_V",
    "trimmed_hole_toggle",
    "surface_grid_U_1_2",
    "surface_grid_U_2_3",
    "surface_grid_V_1_1",
    "surface_grid_V_2_1",
];
