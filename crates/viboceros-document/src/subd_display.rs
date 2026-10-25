//! Document-wide appearance is transient; geometry history never restores it.
use super::Document;

impl Document {
    pub fn subd_control_net(&self) -> bool {
        self.subd_control_net
    }

    /// Select flat control-net or smooth limit-surface display without changing
    /// source geometry, selection, or Undo/Redo availability.
    pub fn set_subd_control_net(&mut self, value: bool) {
        self.subd_control_net = value;
    }
}
