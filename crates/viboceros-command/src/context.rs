//! View-dependent command state, separate from model objects and undo history.

use viboceros_geometry::{Frame3, Point3, Tolerance, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandContext {
    /// Points in command arguments are world-space; this frame supplies the
    /// orientation for construction-plane-dependent operations.
    pub construction_plane: Frame3,
}

impl Default for CommandContext {
    fn default() -> Self {
        Self {
            construction_plane: Frame3::try_from_directions(
                Point3::try_new(0.0, 0.0, 0.0).expect("finite origin"),
                Vector3::try_new(1.0, 0.0, 0.0).expect("finite axis"),
                Vector3::try_new(0.0, 1.0, 0.0).expect("finite axis"),
                Tolerance::DEFAULT,
            )
            .expect("orthogonal world axes"),
        }
    }
}
