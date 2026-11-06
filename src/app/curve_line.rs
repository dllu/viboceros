//! Stateful curve-contact line getters with no temporary document objects.
use super::*;
use viboceros_command::line::curve_input::{Input, Kind, Stage};
mod reference;
#[derive(Clone)]
pub(super) struct Session {
    pub input: Input,
    pub pointer: Option<egui::Pos2>,
    pub binding: Option<(ObjectId, Option<usize>)>,
}
impl VibocerosApp {
    pub(in crate::app) fn curve_line_error(&mut self, error: impl std::fmt::Display) {
        self.push_log(format!("Error: {error}"));
        if self.command_file_job.is_some() {
            self.command_file_step_failed = true;
        }
    }
    pub(super) fn try_start_curve_line(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|s| {
            !matches!(
                s.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "line" | "l"
            )
        }) || viboceros_command::line::curve_input::requested(&words[1..]).is_none()
        {
            return false;
        }
        let kind = viboceros_command::line::curve_input::requested(&words[1..]).unwrap();
        let mut input = Input::with_kind(
            kind,
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        for word in &words[1..] {
            if Kind::parse(word) == Some(kind) {
                continue;
            }
            if !matches!(input.option(word), Ok(true)) {
                return false;
            }
        }
        self.begin_curve_line(input);
        self.push_log(format!("> {text}"));
        true
    }
    pub(super) fn choose_curve_line(&mut self, text: &str) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(kind) = Kind::parse(text) else {
            return false;
        };
        let Some(old) = self
            .line_input
            .filter(|i| i.mode() == viboceros_command::line::Mode::Default && i.anchor().is_none())
        else {
            return false;
        };
        let mut input = Input::with_kind(kind, old.plane(), self.document.tolerance());
        if old.both_sides() {
            input.option("BothSides").unwrap();
        }
        self.begin_curve_line(input);
        true
    }
    fn begin_curve_line(&mut self, mut input: Input) {
        input.set_drafting(
            self.viewports[self.active_viewport].construction_plane(),
            self.planar,
        );
        self.cancel_interactive_command(false);
        self.curve_line = Some(Session {
            input,
            pointer: None,
            binding: None,
        });
        self.curve_line_prompt();
    }
    pub(super) fn curve_line_prompt(&mut self) {
        let Some(s) = &self.curve_line else {
            return;
        };
        if s.input.clears_selection() {
            self.document.clear_selection();
        }
        self.active_command = Some(InteractiveCommand::Line {
            start: s.input.anchor(),
        });
        let prompt = s.input.prompt();
        let name = s.input.kind().name();
        self.push_log(format!("Line {name}: {prompt}; Esc cancels"));
    }
    pub(super) fn validate_curve_line(&mut self) {
        if let Some(s) = &mut self.curve_line {
            s.input.set_drafting(
                self.viewports[self.active_viewport].construction_plane(),
                self.planar,
            );
        }
        if self
            .curve_line
            .as_ref()
            .is_none_or(|s| s.input.current(&self.document))
        {
            return;
        }
        let s = self.curve_line.as_mut().unwrap();
        s.input.reset(self.document.tolerance());
        s.binding = None;
        self.line_accepted_point = None;
        self.point_constraint = None;
        self.point_filter = None;
        self.component_selection.clear_choices();
        self.push_log("Curve reference changed or became unavailable; pick it again".into());
        self.curve_line_prompt();
    }
    pub(super) fn try_continue_curve_line(&mut self, text: &str) -> bool {
        if self.curve_line.is_none() || self.plane_prompt.is_some() || self.object_prompt.is_some()
        {
            return false;
        }
        self.validate_curve_line();
        if text.is_empty() {
            if self.curve_line.as_mut().unwrap().input.enter() {
                self.curve_line_prompt();
                return true;
            }
            self.cancel_interactive_command(false);
            if self.command_file_job.is_some() {
                self.command_file_step_failed = true;
            }
            return true;
        }
        if self.try_curve_line_binding(text) {
            return true;
        }
        let input = &mut self.curve_line.as_mut().unwrap().input;
        match input.option(text) {
            Ok(true) => {
                self.curve_line_prompt();
                return true;
            }
            Err(e) => {
                self.curve_line_error(e);
                return true;
            }
            Ok(false) => {}
        }
        if input.endpoint() {
            let number = text.parse::<f64>().ok().map(Ok).or_else(|| {
                viboceros_drafting::PointInput::parse_length_with_units(text, self.document.units())
            });
            if let Some(value) = number {
                let result = value
                    .map_err(|e| e.to_string())
                    .and_then(|n| input.number(n).map_err(|e| e.to_string()));
                match result {
                    Ok(Some(d)) => {
                        self.finish_line_definition(d);
                        self.line_accepted_point = None;
                    }
                    Ok(None) => self.curve_line_prompt(),
                    Err(e) => self.curve_line_error(e),
                }
                return true;
            }
        }
        false
    }
    pub(super) fn accept_curve_line_point(&mut self, point: Point3) -> bool {
        self.validate_curve_line();
        let Some(s) = &mut self.curve_line else {
            return false;
        };
        if let Some((id, edge)) = s.binding.take() {
            return self.pick_curve_line_at(id, edge, point);
        }
        let result = s.input.point(point);
        match result {
            Ok(Some(d)) => self.finish_line_definition(d),
            Ok(None) => {
                self.line_accepted_point = self.curve_line.as_ref().unwrap().input.anchor();
                self.curve_line_prompt();
                true
            }
            Err(e) => {
                self.curve_line_error(e);
                false
            }
        }
    }
    pub(super) fn curve_line_base_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.curve_line.as_ref()?.input.base_curve()
    }
    pub(super) fn accept_curve_line_parameter(&mut self, t: f64) -> bool {
        let Some(c) = self.curve_line_base_curve() else {
            return false;
        };
        let Ok(p) = c.evaluate(t) else {
            return false;
        };
        let accepted = self.accept_curve_line_point(p);
        if accepted {
            self.last_point = self
                .curve_line
                .as_ref()
                .and_then(|s| s.input.anchor())
                .or(self.last_point);
            self.line_accepted_point = None;
        }
        accepted
    }
    pub(super) fn picking_curve_line(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .curve_line
                .as_ref()
                .is_some_and(|s| s.input.requires_curve() && s.binding.is_none())
    }
}
