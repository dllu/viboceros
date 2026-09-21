//! Reconstruct public prompt-time camera data without reading result positions.
use super::*;
pub(super) use crate::snap_calibration::projection;

pub(super) fn model_history(value: &Value) -> Value {
    let mut expected = value.clone();
    for key in [
        "pick_frames",
        "command_events",
        "command_history",
        "undo_events",
        "redo_events",
        "undo_event_snapshot",
        "redo_event_snapshot",
    ] {
        expected.as_object_mut().unwrap().remove(key);
    }
    expected
}
