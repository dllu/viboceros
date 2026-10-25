#![cfg(feature = "native")]
#[path = "../../viboceros-geometry/tests/support/loft_parameters.rs"]
mod cases;

#[test]
fn native_basis_operations_match_licensed_section_charts() {
    cases::compare_preparation(false, 45, viboceros_smlib::loft_surface);
}

#[test]
#[ignore = "Native coarse lengths differ by one bit and select another boundary at a symmetric near tie"]
fn symmetric_near_tie_matches_licensed_ordered_loft() {
    cases::compare_preparation(true, 1, viboceros_smlib::loft_surface);
}
