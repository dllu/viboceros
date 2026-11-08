//! Nonmeeting curved-source command results are compared against independently captured Rhino.
#[path = "support/fillet_getter.rs"]
#[allow(dead_code)]
mod inputs;
#[path = "support/plane_surface.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/line_reference.rs"]
#[allow(dead_code)]
mod reference_inputs;

#[path = "support/chamfer_getter.rs"]
#[allow(dead_code)]
mod getters;

#[path = "support/chamfer_replay.rs"]
#[allow(dead_code)]
mod replay;
const FAMILIES: &[&str] = &[
    "chamfer_extension_pilot",
    "chamfer_extension_holdouts",
    "chamfer_extension_styling",
    "chamfer_extension_verification",
];
#[test]
fn extensions_match_native_geometry_identity_metadata_and_history() {
    assert_eq!(replay::replay(FAMILIES, "CHAMFER_EXTENSION"), 29);
}
#[test]
fn native_failures_and_unqualified_styles_are_atomic_and_preserve_pending_redo() {
    assert_eq!(replay::rejections(FAMILIES, "CHAMFER_EXTENSION"), 7);
}
