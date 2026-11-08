//! Closed-source command results are compared against independently captured Rhino.
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
    "chamfer_closed_pilot",
    "chamfer_closed_verification",
    "chamfer_closed_choices",
];
#[test]
fn closed_sources_match_native_geometry_identity_metadata_and_history() {
    assert_eq!(replay::replay(FAMILIES, "CHAMFER_CLOSED"), 28);
}
#[test]
fn native_rejections_are_atomic_and_preserve_pending_redo() {
    assert_eq!(replay::rejections(FAMILIES, "CHAMFER_CLOSED"), 4);
}
