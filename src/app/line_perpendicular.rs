//! Stateful perpendicular getters with no temporary document objects.
use super::*;
use viboceros_command::line::perpendicular::{Input, Stage};
mod reference;
#[derive(Clone)]
pub(super) struct Session {
    pub input: Input,
    pub pointer: Option<egui::Pos2>,
    pub binding: Option<(ObjectId, Option<usize>)>,
}
impl VibocerosApp {
    pub(in crate::app) fn perpendicular_error(&mut self, error: impl std::fmt::Display) {
        self.push_log(format!("Error: {error}"));
        if self.command_file_job.is_some() {
            self.command_file_step_failed = true;
        }
    }
    pub(super) fn try_start_line_perpendicular(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|s| {
            !matches!(
                s.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "line" | "l"
            )
        }) || !viboceros_command::line::perpendicular::requested(&words[1..])
        {
            return false;
        }
        let mut input = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.document.tolerance(),
        );
        for word in &words[1..] {
            if word
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Perpendicular")
            {
                continue;
            }
            if !matches!(input.option(word), Ok(true)) {
                return false;
            }
        }
        self.begin_line_perpendicular(input);
        self.push_log(format!("> {text}"));
        true
    }
    pub(super) fn choose_line_perpendicular(&mut self, text: &str) -> bool {
        if self.plane_prompt.is_some()
            || !text
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Perpendicular")
        {
            return false;
        }
        let Some(old) = self
            .line_input
            .filter(|i| i.mode() == viboceros_command::line::Mode::Default && i.anchor().is_none())
        else {
            return false;
        };
        let mut input = Input::new(old.plane(), self.document.tolerance());
        if old.both_sides() {
            input.option("BothSides").unwrap();
        }
        self.begin_line_perpendicular(input);
        true
    }
    fn begin_line_perpendicular(&mut self, mut input: Input) {
        input.set_drafting(
            self.viewports[self.active_viewport].construction_plane(),
            self.planar,
        );
        self.cancel_interactive_command(false);
        self.line_perpendicular = Some(Session {
            input,
            pointer: None,
            binding: None,
        });
        self.line_perpendicular_prompt();
    }
    pub(super) fn line_perpendicular_prompt(&mut self) {
        let Some(s) = &self.line_perpendicular else {
            return;
        };
        if s.input.clears_selection() {
            self.document.clear_selection();
        }
        self.active_command = Some(InteractiveCommand::Line {
            start: s.input.anchor(),
        });
        let prompt = s.input.prompt();
        self.push_log(format!("Line Perpendicular: {prompt}; Esc cancels"));
    }
    pub(super) fn validate_line_perpendicular(&mut self) {
        if let Some(s) = &mut self.line_perpendicular {
            s.input.set_drafting(
                self.viewports[self.active_viewport].construction_plane(),
                self.planar,
            );
        }
        if self
            .line_perpendicular
            .as_ref()
            .is_none_or(|s| s.input.current(&self.document))
        {
            return;
        }
        let s = self.line_perpendicular.as_mut().unwrap();
        s.input.reset(self.document.tolerance());
        s.binding = None;
        self.line_accepted_point = None;
        self.point_constraint = None;
        self.point_filter = None;
        self.component_selection.clear_choices();
        self.push_log(
            "Perpendicular reference changed or became unavailable; pick it again".into(),
        );
        self.line_perpendicular_prompt();
    }
    pub(super) fn try_continue_line_perpendicular(&mut self, text: &str) -> bool {
        if self.line_perpendicular.is_none()
            || self.plane_prompt.is_some()
            || self.object_prompt.is_some()
        {
            return false;
        }
        self.validate_line_perpendicular();
        if text.is_empty() {
            if self.line_perpendicular.as_mut().unwrap().input.enter() {
                self.line_perpendicular_prompt();
                return true;
            }
            self.cancel_interactive_command(false);
            if self.command_file_job.is_some() {
                self.command_file_step_failed = true;
            }
            return true;
        }
        if self.try_line_perpendicular_binding(text) {
            return true;
        }
        let input = &mut self.line_perpendicular.as_mut().unwrap().input;
        match input.option(text) {
            Ok(true) => {
                self.line_perpendicular_prompt();
                return true;
            }
            Err(e) => {
                self.perpendicular_error(e);
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
                    Ok(()) => self.line_perpendicular_prompt(),
                    Err(e) => self.perpendicular_error(e),
                }
                return true;
            }
        }
        false
    }
    pub(super) fn accept_line_perpendicular_point(&mut self, point: Point3) -> bool {
        self.validate_line_perpendicular();
        let Some(s) = &mut self.line_perpendicular else {
            return false;
        };
        if let Some((id, edge)) = s.binding.take() {
            return self.pick_line_perpendicular_at(id, edge, point);
        }
        let result = s.input.point(point);
        match result {
            Ok(Some(d)) => self.finish_line_definition(d),
            Ok(None) => {
                self.line_accepted_point = self.line_perpendicular.as_ref().unwrap().input.anchor();
                self.line_perpendicular_prompt();
                true
            }
            Err(e) => {
                self.perpendicular_error(e);
                false
            }
        }
    }
    pub(super) fn line_perpendicular_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        self.line_perpendicular.as_ref()?.input.base_curve()
    }
    pub(super) fn accept_line_perpendicular_parameter(&mut self, t: f64) -> bool {
        let Some(c) = self.line_perpendicular_curve() else {
            return false;
        };
        let Ok(p) = c.evaluate(t) else {
            return false;
        };
        let accepted = self.accept_line_perpendicular_point(p);
        if accepted {
            self.last_point = self
                .line_perpendicular
                .as_ref()
                .and_then(|s| s.input.anchor())
                .or(self.last_point);
            self.line_accepted_point = None;
        }
        accepted
    }
    pub(super) fn picking_line_perpendicular(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .line_perpendicular
                .as_ref()
                .is_some_and(|s| s.input.requires_curve() && s.binding.is_none())
    }
}
