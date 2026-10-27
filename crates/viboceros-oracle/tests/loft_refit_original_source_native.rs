//! Original-source geometry proposals retain explicit native counterexamples.
use viboceros_geometry::try_propose_joined_refit;
#[path = "support/loft_refit_proposals.rs"]
mod proposals;

fn compare(family: &str, count: usize, expected: (usize, usize, usize)) {
    proposals::compare(
        family,
        count,
        expected,
        "ORIGINAL_SOURCE",
        try_propose_joined_refit,
    );
}

#[test]
fn curved_proposals_match_native_styles_commands_and_source_chart_variants() {
    compare("loft_refit_adaptive_curved", 48, (48, 0, 0));
}

#[test]
fn original_length_lookup_retains_all_curved_clock_transitions() {
    compare("loft_refit_curved_clocks", 12, (12, 0, 0));
}

#[test]
fn source_lookup_keeps_the_prior_scale_and_curvature_matches() {
    compare("loft_refit_join_scales", 20, (20, 0, 0));
    compare("loft_refit_join_curvature", 11, (11, 0, 0));
}

#[test]
fn source_lookup_does_not_qualify_unequal_polyline_subdivision() {
    compare("loft_refit_unequal_sampling", 24, (7, 17, 0));
}

#[test]
fn independent_smooth_rational_spatial_and_quintic_shapes_keep_the_remaining_gaps() {
    compare("loft_refit_adaptive_shapes", 12, (0, 2, 10));
}
