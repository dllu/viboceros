//! Plane's Enter behavior distinguishes defaults from live pointer choices.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnterAction {
    Cancel,
    Dimension,
    Cursor,
}
impl RectangleInput {
    /// Plane's free orientation getter rotates about the second edge point.
    /// Its unconstrained first edge uses the original CPlane elevation.
    pub fn plane_surface_mouse_plane(self) -> Option<Frame3> {
        if matches!(
            self.mode,
            RectangleMode::ThreePoint | RectangleMode::EdgeMidpoint
        ) {
            return match self.points {
                [Some(first), Some(second)] => Frame3::try_from_normal(
                    second,
                    first.vector_to(second).ok()?,
                    Tolerance::NUMERICAL_VALIDATION,
                )
                .ok(),
                [Some(_), None] => Some(self.plane),
                _ => None,
            };
        }
        self.mouse_plane()
    }
    pub fn enter_action(self) -> EnterAction {
        if self.rounding_pending() {
            return EnterAction::Dimension;
        }
        if self.width.is_some() {
            return if matches!(
                self.mode,
                RectangleMode::ThreePoint | RectangleMode::EdgeMidpoint
            ) {
                EnterAction::Cursor
            } else {
                EnterAction::Cancel
            };
        }
        if self.length.is_some() {
            return if matches!(self.mode, RectangleMode::Corners | RectangleMode::Center) {
                EnterAction::Dimension
            } else {
                EnterAction::Cursor
            };
        }
        if self.points[1].is_some() {
            EnterAction::Dimension
        } else {
            EnterAction::Cancel
        }
    }
}
