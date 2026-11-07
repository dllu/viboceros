//! Ordered curve picks and cached ArcBlend preview outside document history.
use super::*;
mod adjustment;
mod radius;
use viboceros_command::arc_blend::Input;
#[derive(Clone)]
pub(super) struct Session {
    pub input: Input,
    pub pointer: Option<egui::Pos2>,
    binding: Option<(ObjectId, Option<usize>)>,
    candidate: Option<(f64, Input)>,
    pub(super) radius_prompt: bool,
}
impl Session {
    pub(super) fn cursor_curve(
        &self,
        plane_prompt_active: bool,
    ) -> Option<&viboceros_geometry::NurbsCurve> {
        if plane_prompt_active {
            None
        } else {
            self.input.adjustment_curve()
        }
    }
    pub(super) fn preview_input(&self) -> &Input {
        self.candidate
            .as_ref()
            .map_or(&self.input, |(_, input)| input)
    }
}
impl VibocerosApp {
    pub(super) fn try_start_arc_blend(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("ArcBlend")
        }) {
            return false;
        }
        if words[1..]
            .iter()
            .any(|w| viboceros_command::line::normal::parse_binding(w).is_some())
        {
            return false;
        }
        let mut input = Input::new(self.document.tolerance());
        for w in &words[1..] {
            match input.option(w) {
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
        self.arc_blend_session = Some(Session {
            input,
            pointer: None,
            binding: None,
            candidate: None,
            radius_prompt: false,
        });
        self.active_command = Some(InteractiveCommand::ArcBlend);
        self.push_log(format!("> {text}"));
        self.arc_blend_prompt();
        true
    }
    fn arc_blend_prompt(&mut self) {
        if let Some(s) = &self.arc_blend_session {
            let prompt = if s.radius_prompt {
                "Radius difference"
            } else {
                s.input.prompt()
            };
            let warning = s.input.warning();
            let message = format!("ArcBlend: {prompt}; Esc cancels");
            if let Some(warning) = warning {
                self.push_log(warning.into());
            }
            self.push_log(message);
        }
    }
    pub(super) fn validate_arc_blend(&mut self) {
        if self
            .arc_blend_session
            .as_ref()
            .is_some_and(|s| !s.input.current(&self.document))
        {
            let s = self.arc_blend_session.as_mut().unwrap();
            s.input.reset(self.document.tolerance());
            s.binding = None;
            s.candidate = None;
            s.radius_prompt = false;
            self.push_log("ArcBlend reference changed; pick the curves again".into());
            self.arc_blend_prompt();
        }
    }
    pub(super) fn picking_arc_blend(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .arc_blend_session
                .as_ref()
                .is_some_and(|s| !s.input.ready())
    }
    pub(super) fn pick_arc_blend_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        self.validate_arc_blend();
        let Some(s) = &mut self.arc_blend_session else {
            return false;
        };
        let result = s.input.pick_curve(&self.document, id, edge, point);
        s.binding = None;
        match result {
            Ok(()) => self.arc_blend_prompt(),
            Err(e) => self.push_log(format!("Error: {e}")),
        };
        true
    }
    pub(super) fn pick_arc_blend_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        if !self.picking_arc_blend() {
            return self.arc_blend_session.is_some();
        }
        let p = id.and_then(|id| {
            self.viewports[self.active_viewport].curve_pick_point(
                self.document.object(id)?.geometry(),
                edge,
                self.arc_blend_session.as_ref()?.pointer?,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(p)) = (id, p) {
            self.pick_arc_blend_at(id, edge, p);
        }
        true
    }
    pub(super) fn continue_arc_blend(&mut self, text: &str) -> bool {
        if self.arc_blend_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if matches!(text, "!" | "Cancel" | "_Cancel") {
            self.cancel_interactive_command(false);
            return true;
        }
        self.validate_arc_blend();
        if self.continue_arc_blend_radius(text) {
            return true;
        }
        if self
            .arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .active_handle()
            .is_some()
        {
            return self.continue_arc_blend_adjustment(text);
        }
        if text.is_empty() {
            let arguments = self.arc_blend_session.as_ref().unwrap().input.arguments();
            match arguments.and_then(|args| self.commands.execute(&mut self.document, &args)) {
                Ok(message) => {
                    self.cancel_interactive_command(false);
                    self.push_log(message);
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            return true;
        }
        let words = text.split_whitespace().collect::<Vec<_>>();
        if let Some((id, kind)) = words
            .first()
            .and_then(|w| viboceros_command::line::normal::parse_binding(w))
        {
            let edge = match kind {
                viboceros_command::line::normal::Component::Object => None,
                viboceros_command::line::normal::Component::Edge(e) => Some(e),
                _ => {
                    self.push_log("ArcBlend requires a curve or edge".into());
                    return true;
                }
            };
            self.arc_blend_session.as_mut().unwrap().binding = Some((id, edge));
            if words.len() == 1 {
                return true;
            }
            return self.arc_blend_binding_point(&words[1..].join(" "));
        }
        if self.arc_blend_session.as_ref().unwrap().binding.is_some() {
            return self.arc_blend_binding_point(text);
        }
        let mut next = self.arc_blend_session.as_ref().unwrap().input.clone();
        for word in words {
            match next.option(word) {
                Ok(true) => {}
                Ok(false) => {
                    self.push_log(format!("Error: unknown ArcBlend input {word}"));
                    return true;
                }
                Err(e) => {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
            }
        }
        self.arc_blend_session.as_mut().unwrap().input = next;
        self.arc_blend_prompt();
        true
    }
    fn arc_blend_binding_point(&mut self, text: &str) -> bool {
        let p = viboceros_drafting::PointInput::parse_with_units(text, self.document.units())
            .and_then(Result::ok)
            .and_then(|p| {
                p.resolve(
                    self.viewports[self.active_viewport].construction_plane(),
                    self.last_point,
                )
                .ok()
            });
        if let Some(p) = p {
            let (id, edge) = self
                .arc_blend_session
                .as_mut()
                .unwrap()
                .binding
                .take()
                .unwrap();
            self.pick_arc_blend_at(id, edge, p);
        } else {
            self.push_log("Error: expected an ArcBlend source pick".into());
        }
        true
    }
}
