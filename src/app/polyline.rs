//! Interactive Polyline stages and their single document edit.
use super::*;
use viboceros_command::polyline_input::{Input, OptionResult};
use viboceros_geometry::{GeometryError, UnitVector3};

pub(super) struct Session {
    pub(super) input: Input,
    pub(super) direction_lock: Option<UnitVector3>,
    cursor: Option<Point3>,
}

impl VibocerosApp {
    pub(super) fn try_start_polyline(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|word| {
            let word = word.trim_start_matches(['_', '-']);
            word.eq_ignore_ascii_case("Polyline") || word.eq_ignore_ascii_case("PLine")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.polyline_preferences(),
        );
        if !words.all(|word| {
            matches!(
                input.option(word, self.document.units(), self.document.tolerance()),
                Ok(Some(OptionResult::Updated))
            )
        }) {
            return false;
        }
        self.cancel_interactive_command(false);
        self.active_command = Some(InteractiveCommand::Polyline);
        self.polyline_session = Some(Session {
            input,
            direction_lock: None,
            cursor: None,
        });
        self.polyline_prompt();
        true
    }

    fn polyline_prompt(&mut self) {
        let Some(session) = &self.polyline_session else {
            return;
        };
        self.commands
            .remember_polyline_preferences(session.input.preferences());
        self.push_log(session.input.prompt());
        self.command_input.clear();
    }

    pub(super) fn continue_polyline(&mut self, text: &str) -> bool {
        let Some(session) = self.polyline_session.as_mut() else {
            return false;
        };
        let tolerance = self.document.tolerance();
        if text.is_empty() {
            let result = session.input.finish(false, tolerance).map(Some);
            self.advance_polyline(result);
            return true;
        }
        match session.input.option(text, self.document.units(), tolerance) {
            Ok(Some(OptionResult::Finish { close })) => {
                let result = session.input.finish(close, tolerance).map(Some);
                self.advance_polyline(result);
                return true;
            }
            Ok(Some(OptionResult::Undo)) => {
                let anchor = session.input.anchor();
                session.direction_lock = None;
                session.cursor = None;
                self.last_point = anchor;
                self.drafting_plane = anchor.map(|_| session.input.plane());
                self.point_filter = None;
                self.point_constraint = None;
                self.polyline_prompt();
                return true;
            }
            Ok(Some(OptionResult::Updated)) => {
                self.point_filter = None;
                self.point_constraint = None;
                session.direction_lock = None;
                self.polyline_prompt();
                return true;
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
            Ok(None) => {}
        }
        if text == "0" && session.input.origin_shortcut() {
            let point = self.viewports[self.active_viewport]
                .construction_plane()
                .origin();
            self.accept_filtered_drafting_point(point, true);
            return true;
        }
        let Some(value) = session.input.parse_value(text, self.document.units()) else {
            return false;
        };
        let value = match value {
            Ok(value) => value,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        let before = session.input.segments().len();
        let result = session.input.value(value, tolerance);
        if session.input.segments().len() != before {
            self.last_point = session.input.anchor();
            self.point_filter = None;
            self.point_constraint = None;
            session.direction_lock = None;
            session.cursor = None;
        }
        self.advance_polyline(result);
        true
    }

    pub(super) fn accept_polyline_point(&mut self, point: Point3) -> Option<bool> {
        let session = self.polyline_session.as_mut()?;
        session
            .input
            .set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = session.input.pick(point, self.document.tolerance());
        if result.is_ok() {
            session.direction_lock = None;
            session.cursor = None;
            self.drafting_plane = Some(session.input.plane());
        }
        Some(self.advance_polyline(result))
    }

    fn advance_polyline(&mut self, result: Result<Option<Geometry>, GeometryError>) -> bool {
        let geometry = match result {
            Ok(Some(geometry)) => geometry,
            Ok(None) => {
                self.polyline_prompt();
                return true;
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_file.fail_dispatch();
                return false;
            }
        };
        let result = (|| {
            self.document.begin_transaction("Polyline")?;
            match self.document.add_geometry(geometry) {
                Ok(id) => {
                    self.document.commit_transaction()?;
                    Ok(id)
                }
                Err(error) => {
                    self.document.rollback_transaction()?;
                    Err(error)
                }
            }
        })();
        match result {
            Ok(id) => {
                self.cancel_interactive_command(false);
                self.command_input.clear();
                self.push_log(format!("Added polyline {id}"));
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_file.fail_dispatch();
                false
            }
        }
    }

    pub(super) fn observe_polyline_cursor(&mut self, point: Point3) {
        if self.plane_prompt.is_none()
            && let Some(s) = &mut self.polyline_session
        {
            s.cursor = Some(point);
        }
    }

    pub(super) fn handle_polyline_tab(&mut self, ui: &egui::Ui) {
        let Some(session) = self.polyline_session.as_mut() else {
            return;
        };
        if !self.command_input.is_empty()
            || self.plane_prompt.is_some()
            || self.point_filter.is_some()
            || self.point_constraint.is_some()
            || !self.viewports[self.active_viewport]
                .screen_rect()
                .zip(ui.input(|input| input.pointer.hover_pos()))
                .is_some_and(|(rect, pointer)| rect.contains(pointer))
        {
            return;
        }
        let direction = session
            .input
            .anchor()
            .zip(session.cursor)
            .and_then(|(anchor, cursor)| anchor.direction_to(cursor).ok());
        if (session.direction_lock.is_some() || direction.is_some())
            && ui.input_mut(|input| {
                command_line::consume_exact_key(input, egui::Modifiers::NONE, egui::Key::Tab)
            })
        {
            session.direction_lock = if session.direction_lock.is_some() {
                None
            } else {
                direction
            };
        }
    }
}
