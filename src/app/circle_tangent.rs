//! Tangent-circle contacts own one command and never select their sources.
use super::*;
use viboceros_command::circle_tangent_input::Input;
pub(super) struct Session {
    pub(super) input: Input,
    pub(super) output: Option<viboceros_command::circle_deform_input::Output>,
    plane: Frame3,
    pub(super) pointer: Option<egui::Pos2>,
}
impl VibocerosApp {
    pub(super) fn picking_circle_tangent(&self) -> bool {
        self.circle_tangent_session
            .as_ref()
            .is_some_and(|s| s.input.wants_curve())
    }
    pub(super) fn start_circle_tangent(
        &mut self,
        output: Option<viboceros_command::circle_deform_input::Output>,
    ) {
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        self.cancel_interactive_command(false);
        self.circle_tangent_session = Some(Session {
            input: Input::new(
                plane,
                self.document.tolerance(),
                self.commands.circle_preferences(),
            ),
            plane,
            output,
            pointer: None,
        });
        self.circle_tangent_prompt();
    }
    pub(super) fn try_start_circle_tangent(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if !matches!(words.as_slice(), [c,t] if ["Circle","C"].iter().any(|w|c.trim_start_matches(['_','-']).eq_ignore_ascii_case(w)) && t.trim_start_matches(['_','-']).eq_ignore_ascii_case("Tangent"))
        {
            return false;
        }
        self.start_circle_tangent(None);
        true
    }
    fn circle_tangent_prompt(&mut self) {
        let Some(s) = &self.circle_tangent_session else {
            return;
        };
        self.active_command = Some(InteractiveCommand::CircleTangent {
            anchor: s.input.anchor(),
        });
        self.drafting_plane = Some(s.plane);
        self.push_log(format!("Circle Tangent: {}; Esc cancels", s.input.prompt()));
    }
    pub(super) fn validate_circle_tangent(&mut self) {
        if self
            .circle_tangent_session
            .as_ref()
            .is_some_and(|s| !s.input.current(&self.document))
        {
            self.circle_tangent_session
                .as_mut()
                .unwrap()
                .input
                .reset_sources(self.document.tolerance());
            self.point_filter = None;
            self.point_constraint = None;
            self.component_selection.clear_choices();
            self.push_log(
                "Circle tangent source changed or became unavailable; pick contacts again".into(),
            );
            self.circle_tangent_prompt();
        }
    }
    fn finish_circle_tangent(&mut self) -> bool {
        let s = self.circle_tangent_session.as_ref().unwrap();
        let command = s.input.command();
        let command = s
            .output
            .map_or(command.clone(), |o| o.wrap_command(&command));
        self.try_execute_command_with_plane(&command, Some(s.plane))
    }
    fn circle_tangent_result(
        &mut self,
        result: Result<Option<viboceros_geometry::Circle3>, viboceros_command::CommandError>,
    ) -> bool {
        match result {
            Ok(Some(_)) => self.finish_circle_tangent(),
            Ok(None) => {
                self.circle_tangent_prompt();
                true
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                false
            }
        }
    }
    pub(super) fn pick_circle_tangent_at(
        &mut self,
        id: ObjectId,
        edge: Option<usize>,
        point: Point3,
    ) -> bool {
        if !self.picking_circle_tangent() {
            return false;
        }
        self.validate_circle_tangent();
        let result = self.circle_tangent_session.as_mut().unwrap().input.curve(
            &self.document,
            id,
            edge,
            point,
        );
        self.component_selection.clear_choices();
        self.circle_tangent_result(result);
        true
    }
    pub(super) fn pick_circle_tangent(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if self.circle_tangent_session.is_none() {
            return false;
        }
        if !self.picking_circle_tangent() {
            return true;
        }
        self.validate_circle_tangent();
        let point = id.and_then(|id| {
            let object = self.document.object(id)?;
            let pointer = self.circle_tangent_session.as_ref()?.pointer?;
            self.viewports[self.active_viewport].curve_pick_point(
                object.geometry(),
                edge,
                pointer,
                self.document.tolerance(),
            )
        });
        if let (Some(id), Some(point)) = (id, point) {
            self.pick_circle_tangent_at(id, edge, point);
        } else {
            self.push_log("Circle Tangent: pick a location on a curve or surface edge".into());
        }
        true
    }
    pub(super) fn accept_circle_tangent_point(&mut self, point: Point3) -> Option<bool> {
        self.circle_tangent_session.as_ref()?;
        self.validate_circle_tangent();
        let result = self
            .circle_tangent_session
            .as_mut()
            .unwrap()
            .input
            .point(point);
        Some(self.circle_tangent_result(result))
    }
    pub(super) fn try_continue_circle_tangent(&mut self, text: &str) -> bool {
        if self.circle_tangent_session.is_none() {
            return false;
        }
        self.validate_circle_tangent();
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
        } else if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let s = self.circle_tangent_session.as_mut().unwrap();
            if s.input.accepts_default() {
                let result = s.input.enter();
                self.circle_tangent_result(result);
            } else {
                self.cancel_interactive_command(true);
            }
        } else if self
            .circle_tangent_session
            .as_mut()
            .unwrap()
            .input
            .option(word)
        {
            self.point_filter = None;
            self.point_constraint = None;
            self.circle_tangent_prompt();
        } else {
            let s = self.circle_tangent_session.as_mut().unwrap();
            if !s.input.wants_number() {
                return false;
            }
            let value = word.strip_prefix("Radius=").unwrap_or(text);
            let Some(n) = viboceros_drafting::PointInput::parse_length_with_units(
                value,
                self.document.units(),
            ) else {
                return false;
            };
            let result = n
                .map_err(|e| e.to_string())
                .and_then(|n| s.input.number(n).map_err(|e| e.to_string()));
            match result {
                Ok(result) => {
                    self.circle_tangent_result(Ok(result));
                }
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        self.command_input.clear();
        true
    }
}
