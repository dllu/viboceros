#![cfg(feature = "native")]
#[path = "../../viboceros-geometry/tests/support/loft_points.rs"]
mod comparison;

#[test]
fn point_ended_lofts_match_native_surfaces_with_the_requested_poles() {
    comparison::compare_end_points(false, viboceros_smlib::loft_surface_with_end_points);
}

#[test]
#[ignore = "native API ignores the end point on four linear-profile records"]
fn linear_native_outputs_that_ignore_the_requested_point_remain_diagnostics() {
    comparison::compare_end_points(true, viboceros_smlib::loft_surface_with_end_points);
}
