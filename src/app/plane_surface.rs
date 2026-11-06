//! Plane owns surface options; point and guide input share Rectangle's getter.
use super::*;
use viboceros_command::plane_surface::{self as solver, Options};
use viboceros_command::rectangle_input::RectangleDefinition;

pub(super) struct Session {
    pub(super) rectangle: rectangle::InputSession,
    options: Options,
    group: viboceros_document::HistoryGroup,
    tolerance: Tolerance,
}
impl VibocerosApp {
    pub(super) fn plane_surface_curve_mode(&mut self) {
        if self
            .plane_surface_session
            .as_ref()
            .is_none_or(|s| !s.rectangle.input.requests_curve())
        {
            return;
        }
        let selected = self
            .document
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .map(|o| o.id())
            .take(2)
            .collect::<Vec<_>>();
        if let [id] = selected.as_slice() {
            self.pick_rectangle_curve(Some(*id), None);
        } else {
            self.document.clear_selection();
        }
    }
    pub(super) fn validate_plane_surface(&mut self) -> bool {
        if self.plane_surface_session.as_ref().is_some_and(|s| {
            !self.document.history_group_is_current(&s.group)
                || s.tolerance != self.document.tolerance()
        }) {
            self.cancel_interactive_command(false);
            self.push_log("Plane inputs or history changed; restart the command".into());
            return false;
        }
        true
    }
    pub(super) fn plane_surface_prompt(&mut self) {
        let s = self.plane_surface_session.as_ref().unwrap();
        let phase = if s.options.pending() {
            s.options.prompt()
        } else {
            let phase = if s.rectangle.cursor_curve().is_some() {
                "center on curve"
            } else {
                s.rectangle.input.prompt()
            };
            format!("{phase}; {}", s.options.prompt())
        };
        self.drafting_plane = s.rectangle.input.mouse_plane();
        self.active_command = Some(InteractiveCommand::Plane {
            first: s.rectangle.input.anchor(),
        });
        self.push_log(format!("Plane: {phase}; Esc cancels"));
    }
    pub(super) fn try_start_plane_surface(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Plane")
        }) {
            return false;
        }
        let mut options = Options::new(self.commands.plane_preferences());
        let mut rectangle =
            rectangle::InputSession::new(self.viewports[self.active_viewport].construction_plane());
        rectangle.input = rectangle.input.with_height_line_constraint();
        for w in words {
            match options.option(w) {
                Ok(true) => (),
                Ok(false) if rectangle.input.option(w) => (),
                _ => return false,
            }
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("Plane") {
            Ok(g) => g,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.commands
            .remember_plane_preferences(options.preferences);
        self.plane_surface_session = Some(Session {
            rectangle,
            options,
            group,
            tolerance: self.document.tolerance(),
        });
        self.plane_surface_curve_mode();
        self.plane_surface_prompt();
        true
    }
    pub(super) fn continue_plane_surface_option(&mut self, text: &str) -> bool {
        let Some(s) = &self.plane_surface_session else {
            return false;
        };
        if !s.options.pending() && s.rectangle.input.anchor().is_some() {
            return false;
        }
        if text
            .trim()
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Cancel")
        {
            return false;
        }
        if !self.validate_plane_surface() {
            return true;
        }
        let s = self.plane_surface_session.as_mut().unwrap();
        match s.options.option(text.trim()) {
            Ok(false) => return false,
            Ok(true) => {
                self.commands
                    .remember_plane_preferences(s.options.preferences);
                self.plane_surface_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    pub(super) fn plane_surface_setting_pending(&self) -> bool {
        self.plane_surface_session
            .as_ref()
            .is_some_and(|s| s.options.pending())
    }
    pub(super) fn finish_plane_surface(&mut self, definition: RectangleDefinition) -> bool {
        if !self.validate_plane_surface() {
            return false;
        }
        let options = self.plane_surface_session.as_ref().unwrap().options;
        if let Err(e) = solver::geometry(definition, options) {
            self.push_log(format!("Error: {e}"));
            return false;
        }
        let [a, b, c] = definition.plane_points();
        self.try_execute_command(&format!(
            "Plane {} 3Point {} {} {}",
            options.arguments(),
            format_model_point(a),
            format_model_point(b),
            format_model_point(c)
        ))
    }
}
