//! Point endpoints share ordinary drafting, snaps, and transient candidate previews.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn wants_blend_curve_point(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .blend_curve_session
                .as_ref()
                .is_some_and(|s| s.point_prompt || s.input.point_adjustment_anchor().is_some())
    }
    pub(in crate::app) fn hover_blend_curve_point(&mut self, point: Point3) {
        self.validate_blend_curve();
        if !self.wants_blend_curve_point() {
            return;
        }
        let s = self.blend_curve_session.as_mut().unwrap();
        let key = CandidateKey::Point(point);
        if s.candidate.as_ref().is_some_and(|(old, _)| *old == key) {
            return;
        }
        let mut next = s.input.clone();
        let result = if s.point_prompt {
            next.pick_point(point)
        } else {
            next.adjust_point(point)
        };
        s.candidate = result.is_ok().then_some((key, next));
    }
    pub(in crate::app) fn accept_blend_curve_point(&mut self, point: Point3) -> Option<bool> {
        if !self.wants_blend_curve_point() {
            return None;
        }
        self.hover_blend_curve_point(point);
        let s = self.blend_curve_session.as_mut().unwrap();
        if let Some((_, mut next)) = s.candidate.take() {
            next.finish_adjustment();
            s.input = next;
            s.point_prompt = false;
            self.blend_curve_prompt();
        } else {
            self.push_log("Error: no valid blend at this point".into());
            return Some(false);
        }
        Some(true)
    }
    pub(in crate::app) fn continue_blend_curve_point(&mut self, text: &str) -> bool {
        let s = self.blend_curve_session.as_ref().unwrap();
        let word = text.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Point")
            && !s.input.ready()
            && !s.point_prompt
            && !s.input.has_point_endpoint()
        {
            let s = self.blend_curve_session.as_mut().unwrap();
            s.point_prompt = true;
            s.binding = None;
            s.candidate = None;
            self.blend_curve_prompt();
            return true;
        }
        if !s.point_prompt {
            return false;
        }
        if let Some(point) = self.blend_curve_point(text) {
            self.accept_filtered_drafting_point(point, true);
        } else {
            self.push_log("Error: pick the point endpoint".into());
        }
        true
    }
}
