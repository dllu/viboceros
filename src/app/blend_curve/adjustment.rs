//! Candidate hover geometry never edits accepted handles or the document.
use super::*;
impl VibocerosApp {
    #[cfg(test)]
    pub(in crate::app) fn select_blend_curve_handle(&mut self, handle: Handle) {
        self.select_blend_curve_handle_with_symmetry(handle, false);
    }
    #[cfg(test)]
    pub(in crate::app) fn select_blend_curve_handle_with_symmetry(
        &mut self,
        handle: Handle,
        symmetric: bool,
    ) {
        self.select_blend_curve_handle_with_modifiers(handle, symmetric, false);
    }
    pub(in crate::app) fn select_blend_curve_handle_with_modifiers(
        &mut self,
        handle: Handle,
        symmetric: bool,
        angle: bool,
    ) {
        self.validate_blend_curve();
        let Some(s) = &mut self.blend_curve_session else {
            return;
        };
        if s.flip || s.subcurve.is_some() {
            return;
        }
        match s
            .input
            .select_handle_with_modifiers(handle, symmetric, angle)
        {
            Ok(()) => {
                s.candidate = None;
                self.blend_curve_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        };
    }
    pub(in crate::app) fn hover_blend_curve_angle_point(&mut self, point: Point3) {
        self.validate_blend_curve();
        let Some(s) = &mut self.blend_curve_session else {
            return;
        };
        if s.input.angle_constraint().is_none() {
            return;
        }
        let key = CandidateKey::Point(point);
        if s.candidate.as_ref().is_some_and(|(old, _)| *old == key) {
            return;
        }
        let mut next = s.input.clone();
        s.candidate = next.adjust_point(point).is_ok().then_some((key, next));
    }
    pub(in crate::app) fn accept_blend_curve_angle_point(&mut self, point: Point3) -> bool {
        if self.plane_prompt.is_some()
            || self
                .blend_curve_session
                .as_ref()
                .is_none_or(|s| s.input.angle_constraint().is_none())
        {
            return false;
        }
        self.hover_blend_curve_angle_point(point);
        let s = self.blend_curve_session.as_mut().unwrap();
        if let Some((_, mut next)) = s.candidate.take() {
            next.finish_adjustment();
            s.input = next;
            self.blend_curve_prompt();
        } else {
            self.push_log("Error: pick a direction away from the blend contact".into());
        }
        true
    }
    pub(in crate::app) fn hover_blend_curve_parameter(&mut self, t: f64) {
        self.validate_blend_curve();
        if self.hover_blend_subcurve_parameter(t) {
            return;
        }
        let Some(s) = &mut self.blend_curve_session else {
            return;
        };
        if s.candidate
            .as_ref()
            .is_some_and(|(old, _)| *old == CandidateKey::Parameter(t))
        {
            return;
        }
        let mut next = s.input.clone();
        let result = if next.shape_constraint().is_some() {
            next.adjust_shape_length(t)
        } else {
            next.adjust_constraint_parameter(t)
        };
        s.candidate = result.is_ok().then_some((CandidateKey::Parameter(t), next));
    }
    pub(in crate::app) fn accept_blend_curve_parameter(&mut self, t: f64) -> bool {
        if self.plane_prompt.is_none() && self.accept_blend_subcurve_parameter(t) {
            return true;
        }
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
        if self.plane_prompt.is_none() && self.discard_blend_subcurve() {
            return true;
        }
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
        if self.wants_blend_curve_point() {
            if let Some(point) = self.blend_curve_point(text) {
                self.accept_filtered_drafting_point(point, true);
            } else {
                self.push_log("Error: pick a blend contact location".into());
            }
            return true;
        }
        let s = self.blend_curve_session.as_ref().unwrap();
        if s.input.secondary_control_adjustment()
            && let Some(distance) =
                viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
        {
            let result = distance
                .map_err(|_| {
                    viboceros_command::CommandError::Usage("Expected a finite control distance")
                })
                .and_then(|v| {
                    self.blend_curve_session
                        .as_mut()
                        .unwrap()
                        .input
                        .set_shape_distance(v)
                });
            match result {
                Ok(()) => {
                    self.blend_curve_session.as_mut().unwrap().candidate = None;
                    self.blend_curve_prompt();
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        let s = self.blend_curve_session.as_ref().unwrap();
        let mut next = s.input.clone();
        let result = if next.angle_constraint().is_some() {
            if let Ok(angle) = text.parse::<f64>() {
                next.adjust_shape_angle(angle)
            } else if let Some(point) = self.blend_curve_point(text) {
                next.adjust_point(point)
            } else {
                Err(viboceros_command::CommandError::Usage(
                    "Enter an angle in degrees or pick a direction",
                ))
            }
        } else if next.shape_constraint().is_some() {
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
