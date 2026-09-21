//! Reconstruct public prompt-time camera data without reading result positions.
use super::*;

pub(super) fn projection(frame: &Value) -> impl Fn(Point3) -> Option<[f64; 2]> {
    let matrix: [[f64; 4]; 4] = serde_json::from_value(frame["world_to_screen"].clone()).unwrap();
    let camera: [f64; 3] = serde_json::from_value(frame["camera_location"].clone()).unwrap();
    let direction: [f64; 3] = serde_json::from_value(frame["camera_direction"].clone()).unwrap();
    move |point: Point3| {
        let p = point.to_array();
        if (0..3)
            .map(|i| (p[i] - camera[i]) * direction[i])
            .sum::<f64>()
            <= 0.
        {
            return None;
        }
        let h: [f64; 4] = std::array::from_fn(|i| {
            matrix[i][3] + (0..3).map(|j| matrix[i][j] * p[j]).sum::<f64>()
        });
        if h[3] == 0. {
            None
        } else {
            Some([h[0] / h[3], h[1] / h[3]])
        }
    }
}

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
