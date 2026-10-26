#[path = "support/refit_join_knots.rs"]
mod support;

#[test]
fn portable_single_removal_matches_the_public_native_sdk() {
    support::compare(viboceros_geometry::NurbsCurve::try_remove_knot_value);
}
