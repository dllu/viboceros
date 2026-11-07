//! Candidate hover geometry never edits accepted handles or the document.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn select_blend_curve_handle(&mut self, handle: Handle) {
        self.validate_blend_curve();
        let Some(s) = &mut self.blend_curve_session else {
            return;
        };
        if s.flip {
            return;
        }
        match s.input.select_handle(handle) {
            Ok(()) => {
                s.candidate = None;
                self.blend_curve_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
    }
    pub(in crate::app) fn hover_blend_curve_parameter(&mut self, t: f64) {
        self.validate_blend_curve();
        let Some(s) = &mut self.blend_curve_session else {
            return;
        };
        if s.candidate.as_ref().is_some_and(|(old, _)| *old == t) {
            return;
        }
        let mut next = s.input.clone();
        let result = if next.shape_constraint().is_some() {
            next.adjust_shape_length(t)
        } else {
            next.adjust_constraint_parameter(t)
        };
        s.candidate = result.is_ok().then_some((t, next));
    }
    pub(in crate::app) fn accept_blend_curve_parameter(&mut self, t: f64) -> bool {
        if self.plane_prompt.is_some()
            || self
                .blend_curve_session
                .as_ref()
                .is_none_or(|s| s.input.active_handle().is_none())
        {
            return false;
        }
        self.hover_blend_curve_parameter(t);
        let s = self.blend_curve_session.as_mut().unwrap();
        if let Some((_, mut next)) = s.candidate.take() {
            next.finish_adjustment();
            s.input = next;
            self.blend_curve_prompt();
        } else {
            self.push_log("Error: no valid blend at this location".into());
        }
        true
    }
    pub(in crate::app) fn cancel_blend_curve_adjustment(&mut self) -> bool {
        let Some(s) = &mut self.blend_curve_session else {
            return false;
        };
        if self.plane_prompt.is_some() || (!s.flip && s.input.active_handle().is_none()) {
            return false;
        }
        s.flip = false;
        s.input.finish_adjustment();
        s.candidate = None;
        self.command_input.clear();
        self.blend_curve_prompt();
        true
    }
    pub(super) fn continue_blend_curve_adjustment(&mut self, text: &str) -> bool {
        if text.is_empty() {
            self.cancel_blend_curve_adjustment();
            return true;
        }
        let s = self.blend_curve_session.as_ref().unwrap();
        let mut next = s.input.clone();
        let result = if next.shape_constraint().is_some() {
            if let Some(length) =
                viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
            {
                length
                    .map_err(|_| {
                        viboceros_command::CommandError::Usage("Expected a finite handle length")
                    })
                    .and_then(|v| next.adjust_shape_length(v))
            } else if let Some(point) = self.blend_curve_point(text) {
                next.adjust_point(point)
            } else {
                Err(viboceros_command::CommandError::Usage(
                    "Pick a blend handle location",
                ))
            }
        } else if let Some(point) = self.blend_curve_point(text) {
            next.adjust_point(point)
        } else {
            Err(viboceros_command::CommandError::Usage(
                "Pick a blend contact location",
            ))
        };
        match result {
            Ok(()) => {
                next.finish_adjustment();
                let s = self.blend_curve_session.as_mut().unwrap();
                s.input = next;
                s.candidate = None;
                self.blend_curve_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
}
