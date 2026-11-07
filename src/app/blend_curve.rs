//! Ordered source picks and a transient adjustable NURBS preview.
use super::*;
use viboceros_command::blend_curve::{Handle, Input};
mod adjustment;
mod picking;
mod points;
mod subcurve;
#[derive(Clone, Copy, PartialEq)]
enum CandidateKey {
    Parameter(f64),
    Point(Point3),
}
#[derive(Clone)]
pub(super) struct Session {
    pub input: Input,
    pub pointer: Option<egui::Pos2>,
    pub flip: bool,
    binding: Option<(ObjectId, Option<usize>)>,
    candidate: Option<(CandidateKey, Input)>,
    pub(super) point_prompt: bool,
    subcurve: Option<crate::app::curve_portion::Editor>,
}
impl Session {
    pub(super) fn preview_input(&self) -> &Input {
        self.candidate.as_ref().map_or(&self.input, |(_, i)| i)
    }
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        if let Some(editor) = &self.subcurve {
            editor.reference.as_ref().map(|r| r.curve())
        } else {
            self.input.adjustment_curve()
        }
    }
}
impl VibocerosApp {
    pub(super) fn try_start_blend_curve(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("BlendCrv")
        }) {
            return false;
        }
        if words[1..].iter().any(|w| {
            viboceros_command::line::normal::parse_binding(w).is_some()
                || viboceros_command::curve_reference::parse_portion_binding(w).is_some()
        }) {
            return false;
        }
        let mut input = Input::with_preferences(
            self.document.tolerance(),
            self.commands.blend_curve_preferences(),
        );
        for w in &words[1..] {
            match input.option_with_units(w, self.document.units()) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        }
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.blend_curve_session = Some(Session {
            input,
            pointer: None,
            flip: false,
            binding: None,
            candidate: None,
            point_prompt: false,
            subcurve: None,
        });
        self.active_command = Some(InteractiveCommand::BlendCurve);
        self.push_log(format!("> {text}"));
        self.blend_curve_prompt();
        true
    }
    fn blend_curve_prompt(&mut self) {
        if let Some(s) = &self.blend_curve_session {
            self.commands
                .remember_blend_curve_preferences(s.input.preferences());
            self.push_log(format!(
                "BlendCrv: {}; Esc cancels",
                if let Some(editor) = &s.subcurve {
                    editor.hint()
                } else if s.flip {
                    "Select an end to flip; Enter flips both"
                } else if s.point_prompt {
                    "End of curve: pick a point"
                } else {
                    s.input.prompt()
                }
            ));
        }
    }
    pub(super) fn validate_blend_curve(&mut self) {
        if self.blend_curve_session.as_ref().is_some_and(|s| {
            !s.input.current(&self.document)
                || s.subcurve
                    .as_ref()
                    .and_then(|p| p.reference.as_ref())
                    .is_some_and(|r| !r.current(&self.document))
        }) {
            let s = self.blend_curve_session.as_mut().unwrap();
            s.input.reset(self.document.tolerance());
            s.binding = None;
            s.candidate = None;
            s.flip = false;
            s.point_prompt = false;
            s.subcurve = None;
            self.push_log("BlendCrv reference changed; pick the sources again".into());
            self.blend_curve_prompt();
        }
    }
    pub(super) fn continue_blend_curve(&mut self, text: &str) -> bool {
        if self.blend_curve_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if text == "!" {
            self.cancel_interactive_command(false);
            return true;
        }
        self.validate_blend_curve();
        if self.continue_blend_curve_subcurve(text) {
            return true;
        }
        let word = text.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            if !self.cancel_blend_curve_adjustment() {
                self.cancel_interactive_command(false);
            }
            return true;
        }
        if self.continue_blend_curve_point(text) {
            return true;
        }
        if self.blend_curve_session.as_ref().unwrap().flip {
            if text.is_empty() || word.eq_ignore_ascii_case("Enter") {
                let s = self.blend_curve_session.as_mut().unwrap();
                if let Err(e) = s.input.flip_both() {
                    self.push_log(format!("Error: {e}"));
                } else {
                    s.flip = false;
                    self.blend_curve_prompt();
                }
                return true;
            }
        } else if self
            .blend_curve_session
            .as_ref()
            .unwrap()
            .input
            .active_handle()
            .is_some()
        {
            return self.continue_blend_curve_adjustment(text);
        } else if text.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let args = self.blend_curve_session.as_ref().unwrap().input.arguments();
            match args.and_then(|a| self.commands.execute(&mut self.document, &a)) {
                Ok(m) => {
                    self.cancel_interactive_command(false);
                    self.push_log(m);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            };
            return true;
        } else if word.eq_ignore_ascii_case("Flip")
            && self.blend_curve_session.as_ref().unwrap().input.ready()
        {
            self.blend_curve_session.as_mut().unwrap().flip = true;
            self.blend_curve_prompt();
            return true;
        }
        if let Some((id, component)) = viboceros_command::line::normal::parse_binding(text) {
            let edge = match component {
                viboceros_command::line::normal::Component::Object => None,
                viboceros_command::line::normal::Component::Edge(e) => Some(e),
                _ => {
                    self.push_log("Select a curve or edge".into());
                    return true;
                }
            };
            self.blend_curve_session.as_mut().unwrap().binding = Some((id, edge));
            self.push_log("Pick the blend source end or location".into());
            return true;
        }
        if let Some((id, edge)) = self.blend_curve_session.as_ref().unwrap().binding {
            if let Some(point) = self.blend_curve_point(text) {
                self.pick_blend_curve_at(id, edge, point);
            } else {
                self.push_log("Error: expected a blend source location".into());
            }
            return true;
        }
        let mut next = self.blend_curve_session.as_ref().unwrap().input.clone();
        for w in text.split_whitespace() {
            match next.option_with_units(w, self.document.units()) {
                Ok(true) => {}
                Ok(false) => {
                    self.push_log(format!("Unknown BlendCrv input: {w}"));
                    return true;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        }
        self.blend_curve_session.as_mut().unwrap().input = next;
        self.blend_curve_prompt();
        true
    }
    fn blend_curve_point(&self, text: &str) -> Option<Point3> {
        viboceros_drafting::PointInput::parse_with_units(text, self.document.units())
            .and_then(Result::ok)?
            .resolve(
                self.viewports[self.active_viewport].construction_plane(),
                self.last_point,
            )
            .ok()
    }
}
