//! Typed numbers and two-point distances share the cached candidate.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn hover_curve_fillet_radius(&mut self, point: Point3) {
        if self.plane_prompt.is_some() {
            return;
        }
        if let Some(start) = self
            .curve_fillet_session
            .as_ref()
            .and_then(|s| s.radius_start)
            && let Ok(radius) = start.distance_to(point)
        {
            self.update_curve_fillet_preview(Some(radius), false);
        }
    }
    pub(in crate::app) fn accept_curve_fillet_point(&mut self, point: Point3) -> Option<bool> {
        if self.plane_prompt.is_some() {
            return None;
        }
        let s = self.curve_fillet_session.as_mut()?;
        if !s.wants_point() {
            return Some(false);
        }
        if let Some(start) = s.radius_start {
            match start.distance_to(point) {
                Ok(radius) => {
                    self.accept_curve_fillet_radius(radius);
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return Some(false);
                }
            }
        } else {
            s.radius_start = Some(point);
            self.active_command = Some(InteractiveCommand::Fillet {
                anchor: Some(point),
            });
            self.curve_fillet_prompt();
        }
        Some(true)
    }
    fn accept_curve_fillet_radius(&mut self, radius: Real) {
        let s = self.curve_fillet_session.as_mut().unwrap();
        s.options.radius = radius;
        s.radius_start = None;
        s.pending = None;
        self.active_command = Some(InteractiveCommand::Fillet { anchor: None });
        self.update_curve_fillet_preview(None, true);
        self.curve_fillet_prompt();
    }
    pub(super) fn continue_curve_fillet_radius(&mut self, text: &str) -> bool {
        let s = self.curve_fillet_session.as_ref().unwrap();
        if s.pending.is_some_and(|p| p != "Radius") {
            return false;
        }
        if text.trim_start_matches('_').eq_ignore_ascii_case("Radius") {
            self.curve_fillet_session.as_mut().unwrap().pending = Some("Radius");
            self.curve_fillet_prompt();
            return true;
        }
        if text.is_empty() && s.pending == Some("Radius") {
            self.curve_fillet_session.as_mut().unwrap().pending = None;
            self.curve_fillet_session.as_mut().unwrap().radius_start = None;
            self.active_command = Some(InteractiveCommand::Fillet { anchor: None });
            self.update_curve_fillet_preview(None, true);
            self.curve_fillet_prompt();
            return true;
        }
        if let Ok(radius) = parse_radius(text, self.document.units()) {
            if s.wants_point() {
                self.accept_curve_fillet_radius(radius);
            } else {
                self.curve_fillet_session.as_mut().unwrap().options.radius = radius;
                self.curve_fillet_prompt();
            }
            return true;
        }
        if s.wants_point()
            && (self.try_continue_point_constraint(text)
                || self.try_continue_point_filter(text)
                || self.try_continue_point_input(text))
        {
            return true;
        }
        false
    }
}
