#![cfg(feature = "native")]
#[path = "../../viboceros-geometry/tests/support/refit_join_knots.rs"]
mod support;

#[test]
fn smlib_single_removal_matches_the_public_native_sdk() {
    support::compare(viboceros_smlib::remove_curve_knot_at_value);
}
