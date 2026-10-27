//! Whole-profile and trimmed references retain strict legacy comparisons.
#[path = "support/loft_refit_admission.rs"]
mod admission;

#[test]
fn subdivision_references_have_complete_native_bounds_and_keep_legacy_records() {
    let stats = admission::compare("loft_refit_subdivision", 0..45);
    // Fourteen old Loft records and all nine public closest-point records.
    assert_eq!(stats.legacy, 23);
}
