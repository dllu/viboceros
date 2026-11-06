//! Shared edit-free Line getters, constraints, and viewport metadata.
use super::*;
use viboceros_command::line::Input;

impl VibocerosApp {
    pub(super) fn try_start_line(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            matches!(
                w.trim_start_matches(['_', '-'])
                    .to_ascii_lowercase()
                    .as_str(),
                "line" | "l"
            )
        }) {
            return false;
        }
        let mut input = Input::new(self.viewports[self.active_viewport].construction_plane());
        for word in words {
            if !matches!(input.option(word), Ok(true)) {
                return false;
            }
        }
        self.cancel_interactive_command(true);
        self.line_input = Some(input);
        self.push_log(format!("> {text}"));
        self.line_prompt();
        true
    }
    fn line_prompt(&mut self) {
        let input = self.line_input.unwrap();
        self.active_command = Some(InteractiveCommand::Line {
            start: input.anchor(),
        });
        self.push_log(format!("Line: {}; Esc cancels", input.prompt()));
    }
    pub(super) fn try_continue_line(&mut self, text: &str) -> bool {
        if self.line_input.is_none() || self.plane_prompt.is_some() || self.object_prompt.is_some()
        {
            return false;
        }
        if text.is_empty() {
            if self.command_file_job.is_some() {
                self.command_file_step_failed = true;
            }
            self.cancel_interactive_command(false);
            return true;
        }
        let input = self.line_input.as_mut().unwrap();
        match input.option(text) {
            Ok(true) => {
                self.line_prompt();
                return true;
            }
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
            Ok(false) => {}
        }
        // Ordinary Line retains the shared drafting distance/angle shortcuts.
        // Construction modes project first and apply their own distance lock.
        if (input.requests_angle() || input.direction().ok().flatten().is_some())
            && let Ok(value) = text.parse::<f64>()
        {
            if value == 0. && !input.requests_angle() {
                return false;
            }
            let result = input.number(value);
            match result {
                Ok(_) => self.line_prompt(),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
    pub(super) fn accept_line_point(&mut self, point: Point3) -> bool {
        if self.line_input.is_none() {
            let mut input = Input::new(
                self.drafting_plane
                    .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane()),
            );
            if let Some(InteractiveCommand::Line { start: Some(start) }) = self.active_command
                && input.point(start).is_err()
            {
                return false;
            }
            self.line_input = Some(input);
        }
        let mut input = self.line_input.unwrap();
        match input.point(point) {
            Ok(Some(definition)) => {
                let command = format!(
                    "Line {} {}",
                    format_model_point(definition.line.start()),
                    format_model_point(definition.line.end())
                );
                if !self.try_execute_command(&command) {
                    self.line_input = Some(input);
                    self.line_prompt();
                    return false;
                }
                self.line_accepted_point = Some(definition.accepted_point);
                true
            }
            Ok(None) if input.cancelled() => {
                self.cancel_interactive_command(false);
                if self.command_file_job.is_some() {
                    self.command_file_step_failed = true;
                }
                true
            }
            Ok(None) => {
                self.line_input = Some(input);
                self.line_accepted_point = input.last_point();
                self.line_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: line end or reference point: {error}"));
                false
            }
        }
    }
    pub(super) fn line_axis_constraint(
        &self,
    ) -> Option<viboceros_command::translation::DestinationConstraint> {
        let input = self.line_input?;
        if input.requests_angle() {
            return None;
        }
        Some(viboceros_command::translation::DestinationConstraint {
            anchor: input.anchor()?,
            direction: Some(input.direction().ok()??),
            distance: None,
        })
    }
}
