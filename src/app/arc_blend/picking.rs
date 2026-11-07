//! Ordered whole-curve picks and explicit source bindings.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn pick_arc_blend_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        self.validate_arc_blend();
        if self
            .arc_blend_session
            .as_ref()
            .is_some_and(|s| s.subcurve.is_some())
        {
            return self.pick_arc_subcurve_reference(id, edge, Some(point));
        }
        let Some(s) = &mut self.arc_blend_session else {
            return false;
        };
        let result = s.input.pick_curve(&self.document, id, edge, point);
        s.binding = None;
        match result {
            Ok(()) => self.arc_blend_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    pub(in crate::app) fn pick_arc_blend_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        if !self.picking_arc_blend() {
            return self.arc_blend_session.is_some();
        }
        let p = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.arc_blend_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(p)) = (id, p) {
            self.pick_arc_blend_at(id, edge, p);
        }
        true
    }
    pub(super) fn arc_blend_binding_point(&mut self, text: &str) -> bool {
        let p = viboceros_drafting::PointInput::parse_with_units(text, self.document.units())
            .and_then(Result::ok)
            .and_then(|p| {
                p.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    self.last_point,
                )
                .ok()
            });
        if let Some(p) = p {
            let (id, edge) = self
                .arc_blend_session
                .as_mut()
                .unwrap()
                .binding
                .take()
                .unwrap();
            self.pick_arc_blend_at(id, edge, p);
        } else {
            self.push_log("Error: expected an ArcBlend source pick".into());
        }
        true
    }
}
