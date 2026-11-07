//! Preview analysis follows the current candidate and ends with its getter.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn blend_curvature_display(
        &self,
    ) -> Option<viboceros_command::blend_curve::CurvatureDisplay> {
        let session = self.blend_curve_session.as_ref()?;
        if !session.input.current(&self.document) {
            return None;
        }
        session
            .preview_input()
            .curvature_display(&self.commands.curvature_graph_options())
            .ok()
            .flatten()
    }
    pub(in crate::app) fn hide_blend_curvature(&mut self) {
        if let Some(session) = &mut self.blend_curve_session {
            if session.input.ready() {
                let _ = session.input.set_curvature_display(false);
            }
            if let Some((_, candidate)) = &mut session.candidate {
                let _ = candidate.set_curvature_display(false);
            }
        }
    }
}
