//! Surface-edge admission retains the immutable parent through both point picks.
use super::*;

impl VibocerosApp {
    pub(in crate::app) fn picking_uv_subcurve_edge(&self) -> bool {
        self.intersection_prompt.as_ref().is_some_and(|p| {
            p.curve_extraction.is_some()
                && p.name() == "ExtractPt"
                && p.uv_subcurves
                    .pending
                    .as_ref()
                    .is_some_and(|s| s.object.is_none())
        })
    }

    pub(in crate::app) fn pick_uv_subcurve_edge(
        &mut self,
        pick: crate::viewport::EdgePick,
    ) -> bool {
        if !self.picking_uv_subcurve_edge() {
            return false;
        }
        let reference = match CapturedCurve::capture(&self.document, pick.object, Some(pick.edge)) {
            Ok(reference) => reference,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let pending = self
            .intersection_prompt
            .as_mut()
            .unwrap()
            .uv_subcurves
            .pending
            .as_mut()
            .unwrap();
        pending.object = Some(pick.object);
        pending.source = Some(reference.source().0.clone());
        pending.edge_reference = Some(reference);
        self.active_command = Some(InteractiveCommand::SubCrv {
            start: None,
            copy: true,
            delete: false,
        });
        self.log_intersection_prompt();
        true
    }
}
