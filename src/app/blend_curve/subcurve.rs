//! Adapt the shared SubCrv getter to the blend_curve source-selection session.
use super::*;
use crate::app::curve_portion::{Action, Editor};
impl Session {
    pub(in crate::app) fn subcurve_hover(&self) -> Option<f64> {
        self.subcurve.as_ref()?.hover()
    }
    pub(in crate::app) fn subcurve_preview(
        &mut self,
        doc: &Document,
    ) -> Option<crate::app::subcurve_preview::Preview> {
        self.subcurve.as_mut()?.preview(doc)
    }
}
impl VibocerosApp {
    pub(in crate::app) fn discard_blend_subcurve(&mut self) -> bool {
        let Some(s) = self.blend_curve_session.as_mut() else {
            return false;
        };
        if s.subcurve.take().is_none() {
            return false;
        }
        s.binding = None;
        s.candidate = None;
        self.command_input.clear();
        self.blend_curve_prompt();
        true
    }
    pub(super) fn pick_blend_subcurve_reference(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> bool {
        let Some(p) = self
            .blend_curve_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
        else {
            return false;
        };
        if p.reference.is_some() {
            return true;
        }
        match p.capture(&self.document, id, edge, pick) {
            Ok(()) => self.blend_curve_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    pub(super) fn hover_blend_subcurve_parameter(&mut self, t: f64) -> bool {
        let Some(p) = self
            .blend_curve_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
        else {
            return false;
        };
        p.hover_parameter(t);
        true
    }
    pub(super) fn accept_blend_subcurve_parameter(&mut self, t: f64) -> bool {
        self.validate_blend_curve();
        let Some(p) = self
            .blend_curve_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
            .filter(|p| p.reference.is_some())
        else {
            return false;
        };
        let result = p.accept_parameter(t, self.document.tolerance());
        if result.is_ok() {
            self.snaps.model_override = None;
        }
        self.blend_subcurve_action(result);
        true
    }
    fn blend_subcurve_action(&mut self, result: Result<Action, String>) {
        match result {
            Ok(Action::Pending) => self.blend_curve_prompt(),
            Ok(Action::Discard) => {
                self.discard_blend_subcurve();
            }
            Ok(Action::Capture(id, edge)) => {
                self.pick_blend_subcurve_reference(id, edge, None);
            }
            Ok(Action::Selected(p)) => {
                let result = self
                    .blend_curve_session
                    .as_mut()
                    .unwrap()
                    .input
                    .pick_subcurve(&self.document, p.object, p.edge, p.parameters, p.wrap);
                match result {
                    Ok(()) => {
                        self.discard_blend_subcurve();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                };
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn continue_blend_curve_subcurve(&mut self, text: &str) -> bool {
        let s = self.blend_curve_session.as_ref().unwrap();
        if text
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("SubCrv")
            && s.subcurve.is_none()
            && !s.input.ready()
            && !s.point_prompt
        {
            let s = self.blend_curve_session.as_mut().unwrap();
            s.subcurve = Some(Editor::default());
            s.binding = None;
            s.candidate = None;
            self.blend_curve_prompt();
            return true;
        }
        let Some(p) = self
            .blend_curve_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
        else {
            return false;
        };
        let result = p.input(
            text,
            &self.document,
            self.viewports[self.active_viewport].construction_plane(),
            self.last_point,
        );
        self.blend_subcurve_action(result);
        true
    }
}
