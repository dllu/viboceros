#[path = "support/section_refit.rs"]
mod section_refit;

#[test]
fn large_tolerances_match_complete_native_curve_images() {
    section_refit::require_complete_family("sweep1_refit_large_tolerances", 50);
}

#[test]
fn large_tolerance_holdouts_match_complete_native_curve_images() {
    section_refit::require_complete_family("sweep1_refit_large_holdouts", 16);
}

#[test]
fn endpoint_frames_match_complete_native_curve_images() {
    section_refit::require_complete_family("sweep1_refit_endpoint_frames", 24);
}

#[test]
fn opposed_endpoint_frames_match_complete_native_curve_images() {
    section_refit::require_complete_family("sweep1_refit_opposed_frames", 18);
}

#[test]
fn opposed_endpoint_boundary_matches_complete_native_curve_images() {
    section_refit::require_complete_family("sweep1_refit_opposed_boundary", 6);
}

#[test]
fn opposed_endpoint_cosines_match_complete_native_curve_images() {
    section_refit::require_complete_family("sweep1_refit_opposed_cosines", 10);
}
