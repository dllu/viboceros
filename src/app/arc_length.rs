//! World-space distance input retains the arc's captured construction plane.
use super::*;
#[derive(Clone, Copy)]
pub(super) struct Session {
    pub first: Option<Point3>,
    pub clockwise: bool,
    pub plane: Frame3,
}
impl VibocerosApp {
    pub(super) fn start_arc_length(&mut self) {
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        self.arc_length_session = Some(Session {
            first: None,
            clockwise: false,
            plane,
        });
        self.drafting_plane = None;
        self.point_constraint = None;
    }
    pub(super) fn accept_arc_length_point(&mut self, point: Point3) -> Option<bool> {
        let s = self.arc_length_session.as_mut()?;
        let Some(first) = s.first else {
            s.first = Some(point);
            self.last_point = Some(point);
            self.push_log("Arc length: second distance point".into());
            return Some(true);
        };
        let distance = match first.distance_to(point) {
            Ok(d) if d > 0. => d,
            _ => {
                self.push_log("Error: arc length must be nonzero".into());
                return Some(false);
            }
        };
        self.last_point = Some(point);
        Some(self.try_continue_arc(&distance.to_string()))
    }
}
