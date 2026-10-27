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

// Shape groups are independent; keep slow exact certificates concurrent.
macro_rules! shape_groups {
    ($($name:ident, $family:literal, $count:literal, $range:expr, $expected:expr;)*) => { $(
        #[test]
        fn $name() {
            proposals::compare_range($family, $count, $range, $expected,
                "ORIGINAL_SOURCE", try_propose_joined_refit);
        }
    )*};
}

shape_groups! {
    smooth_shapes, "loft_refit_adaptive_shapes", 12, 0..3, (2,1,0);
    rational_shapes, "loft_refit_adaptive_shapes", 12, 3..6, (1,2,0);
    spatial_shapes, "loft_refit_adaptive_shapes", 12, 6..9, (2,1,0);
    quintic_shapes, "loft_refit_adaptive_shapes", 12, 9..12, (2,1,0);
    smooth_join_modes, "loft_refit_join_modes", 16, 0..4, (1,3,0);
    rational_join_modes, "loft_refit_join_modes", 16, 4..8, (1,3,0);
    spatial_join_modes, "loft_refit_join_modes", 16, 8..12, (1,3,0);
    quintic_join_modes, "loft_refit_join_modes", 16, 12..16, (4,0,0);
}
