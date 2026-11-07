//! Ordered source picks and a transient adjustable NURBS preview.
use super::*;
use viboceros_command::blend_curve::{Handle, Input};
mod adjustment;
#[derive(Clone)]
pub(super) struct Session {
    pub input: Input,
    pub pointer: Option<egui::Pos2>,
    pub flip: bool,
    binding: Option<(ObjectId, Option<usize>)>,
    candidate: Option<(f64, Input)>,
}
impl Session {
    pub(super) fn preview_input(&self) -> &Input {
        self.candidate.as_ref().map_or(&self.input, |(_, i)| i)
    }
    pub(super) fn cursor_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.input.adjustment_curve()
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
        if words[1..]
            .iter()
            .any(|w| viboceros_command::line::normal::parse_binding(w).is_some())
        {
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
                if s.flip {
                    "Select an end to flip; Enter flips both"
                } else {
                    s.input.prompt()
                }
            ));
        }
    }
    pub(super) fn validate_blend_curve(&mut self) {
        if self
            .blend_curve_session
            .as_ref()
            .is_some_and(|s| !s.input.current(&self.document))
        {
            let s = self.blend_curve_session.as_mut().unwrap();
            s.input.reset(self.document.tolerance());
            s.binding = None;
            s.candidate = None;
            s.flip = false;
            self.push_log("BlendCrv reference changed; pick the sources again".into());
            self.blend_curve_prompt();
        }
    }
    pub(super) fn picking_blend_curve(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .blend_curve_session
                .as_ref()
                .is_some_and(|s| !s.input.ready() || s.flip)
    }
    pub(super) fn pick_blend_curve_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        self.validate_blend_curve();
        let Some(s) = &mut self.blend_curve_session else {
            return false;
        };
        let result = if s.flip {
            s.input.flip_source(id)
        } else {
            s.input.pick_curve(&self.document, id, edge, point)
        };
        if result.is_ok() {
            s.flip = false;
        }
        s.binding = None;
        match result {
            Ok(()) => self.blend_curve_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    pub(super) fn pick_blend_curve(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        if !self.picking_blend_curve() {
            return self.blend_curve_session.is_some();
        }
        let point = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.blend_curve_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(p)) = (id, point) {
            self.pick_blend_curve_at(id, edge, p);
        }
        true
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
        let word = text.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            if !self.cancel_blend_curve_adjustment() {
                self.cancel_interactive_command(false);
            }
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
