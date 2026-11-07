//! Adapt the shared SubCrv getter to the arc_blend source-selection session.
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
    pub(in crate::app) fn discard_arc_subcurve(&mut self) -> bool {
        let Some(s) = self.arc_blend_session.as_mut() else {
            return false;
        };
        if s.subcurve.take().is_none() {
            return false;
        }
        s.binding = None;
        s.candidate = None;
        self.command_input.clear();
        self.arc_blend_prompt();
        true
    }
    pub(super) fn pick_arc_subcurve_reference(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        pick: Option<Point3>,
    ) -> bool {
        let Some(p) = self
            .arc_blend_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
        else {
            return false;
        };
        if p.reference.is_some() {
            return true;
        }
        match p.capture(&self.document, id, edge, pick) {
            Ok(()) => self.arc_blend_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    pub(super) fn hover_arc_subcurve_parameter(&mut self, t: f64) -> bool {
        let Some(p) = self
            .arc_blend_session
            .as_mut()
            .and_then(|s| s.subcurve.as_mut())
        else {
            return false;
        };
        p.hover_parameter(t);
        true
    }
    pub(super) fn accept_arc_subcurve_parameter(&mut self, t: f64) -> bool {
        self.validate_arc_blend();
        let Some(p) = self
            .arc_blend_session
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
        self.arc_subcurve_action(result);
        true
    }
    fn arc_subcurve_action(&mut self, result: Result<Action, String>) {
        match result {
            Ok(Action::Pending) => self.arc_blend_prompt(),
            Ok(Action::Discard) => {
                self.discard_arc_subcurve();
            }
            Ok(Action::Capture(id, edge)) => {
                self.pick_arc_subcurve_reference(id, edge, None);
            }
            Ok(Action::Selected(p)) => {
                let result = self
                    .arc_blend_session
                    .as_mut()
                    .unwrap()
                    .input
                    .pick_subcurve(&self.document, p.object, p.edge, p.parameters, p.wrap);
                match result {
                    Ok(()) => {
                        self.discard_arc_subcurve();
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                };
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    pub(super) fn continue_arc_blend_subcurve(&mut self, text: &str) -> bool {
        let s = self.arc_blend_session.as_ref().unwrap();
        if text
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("SubCrv")
            && s.subcurve.is_none()
            && !s.input.ready()
        {
            let s = self.arc_blend_session.as_mut().unwrap();
            s.subcurve = Some(Editor::default());
            s.binding = None;
            s.candidate = None;
            self.arc_blend_prompt();
            return true;
        }
        let Some(p) = self
            .arc_blend_session
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
        self.arc_subcurve_action(result);
        true
    }
}
