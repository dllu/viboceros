//! Curve references and free point drafting use distinct picking modes.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn picking_blend_curve(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .blend_curve_session
                .as_ref()
                .is_some_and(|s| !s.point_prompt && (!s.input.ready() || s.flip))
    }
    pub(in crate::app) fn pick_blend_curve_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        self.validate_blend_curve();
        let Some(s) = &mut self.blend_curve_session else {
            return false;
        };
        let result = if s.flip {
            s.input.flip_source(id)
        } else {
            s.input.pick_curve(&self.document, id, edge, point)
        };
        if result.is_ok() {
            s.flip = false;
        }
        s.binding = None;
        match result {
            Ok(()) => self.blend_curve_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    pub(in crate::app) fn pick_blend_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        if !self.picking_blend_curve() {
            return self.blend_curve_session.is_some();
        }
        let point = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.blend_curve_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(p)) = (id, point) {
            self.pick_blend_curve_at(id, edge, p);
        }
        true
    }
}
