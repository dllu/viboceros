//! Polygon input, guide selection and atomic document edits.
use super::*;
use viboceros_command::polygon::{Definition, Input};
use viboceros_document::HistoryGroup;
use viboceros_geometry::GeometryError;

pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
    released_guide: Option<ObjectId>,
    cursor: Option<Point3>,
}

impl VibocerosApp {
    pub(super) fn try_start_polygon(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|word| {
            let word = word.trim_start_matches(['_', '-']);
            word.eq_ignore_ascii_case("Polygon") || word.eq_ignore_ascii_case("Poly")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.polygon_preferences(),
            self.document.tolerance(),
        );
        if let Some(word) = words.next() {
            if word.trim_start_matches('_').parse::<usize>().is_ok() {
                if input.set_sides(word).is_err() {
                    return false;
                }
            } else if !input.option(word).is_ok_and(|handled| handled) {
                return false;
            }
        }
        if !words.all(|word| input.option(word).is_ok_and(|handled| handled)) {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("Polygon") {
            Ok(group) => group,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_file.fail_dispatch();
                return true;
            }
        };
        self.polygon_session = Some(Session {
            input,
            group,
            released_guide: None,
            cursor: None,
        });
        if input.needs_guide() {
            self.commands
                .remember_polygon_preferences(input.preferences());
            self.start_polygon_around();
        } else {
            self.polygon_prompt();
        }
        true
    }

    pub(super) fn valid_polygon(&mut self) -> bool {
        if let Some(s) = &self.polygon_session
            && (!self.document.history_group_is_current(&s.group)
                || self.document.tolerance() != s.input.tolerance())
        {
            self.cancel_interactive_command(false);
            self.push_log("Polygon inputs or history changed. Restart the command.".into());
            self.command_file.fail_dispatch();
            return false;
        }
        true
    }

    fn polygon_prompt(&mut self) {
        let input = self.polygon_session.as_ref().unwrap().input;
        self.commands
            .remember_polygon_preferences(input.preferences());
        self.drafting_plane = input.mouse_plane();
        self.active_command = Some(InteractiveCommand::Polygon {
            anchor: input.anchor(),
        });
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        self.push_log(input.prompt());
    }

    pub(super) fn observe_polygon_cursor(&mut self, point: Point3) {
        if self.plane_prompt.is_none()
            && let Some(s) = &mut self.polygon_session
            && !s.input.needs_guide()
            && !s.input.pending_count()
        {
            s.cursor = Some(point);
        }
    }

    pub(super) fn try_polygon_option(&mut self, text: &str) -> bool {
        if text
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
        {
            return false;
        }
        let input = &mut self.polygon_session.as_mut().unwrap().input;
        match input.option(text) {
            Ok(true) => {
                if input.needs_guide() {
                    self.commands
                        .remember_polygon_preferences(input.preferences());
                    self.command_input.clear();
                    if self.circle_around_session.is_some() {
                        self.circle_around_prompt();
                    } else {
                        self.start_polygon_around();
                    }
                } else {
                    self.polygon_prompt();
                }
                true
            }
            Ok(false) => false,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                true
            }
        }
    }

    pub(super) fn try_continue_polygon(&mut self, text: &str) -> bool {
        if self.polygon_session.is_none() || self.circle_around_session.is_some() {
            return false;
        }
        if !self.valid_polygon() {
            return true;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if self.try_polygon_option(text) {
            return true;
        }
        let session = self.polygon_session.as_mut().unwrap();
        let input = &mut session.input;
        let result = if word.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let cursor = session.cursor.and_then(|point| {
                let point = self.point_filter.map_or(Some(point), |filter| {
                    (!filter.awaiting_source())
                        .then(|| filter.preview_point(point).ok())
                        .flatten()
                })?;
                self.point_constraint
                    .map_or(Some(point), |state| state.apply_cursor(point).ok())
            });
            input.enter(cursor)
        } else if text == "0" && input.accepts_origin() {
            let point = self.viewports[self.active_viewport]
                .construction_plane()
                .origin();
            self.accept_filtered_drafting_point(point, true);
            return true;
        } else if input.takes_size() {
            let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                text,
                self.document.units(),
            ) else {
                return false;
            };
            match value {
                Ok(value) => input.number(value),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
            }
        } else {
            return false;
        };
        self.advance_polygon(result);
        true
    }

    pub(super) fn accept_polygon_point(&mut self, point: Point3) -> Option<bool> {
        self.polygon_session.as_ref()?;
        if !self.valid_polygon() {
            return Some(false);
        }
        let session = self.polygon_session.as_mut().unwrap();
        if session.input.pending_count() {
            return Some(false);
        }
        session.cursor = None;
        let input = &mut session.input;
        input.set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = input.point(point);
        Some(self.advance_polygon(result))
    }

    pub(super) fn accept_polygon_guide(
        &mut self,
        frame: Frame3,
        release: Option<ObjectId>,
    ) -> bool {
        self.release_circle_around_picks();
        self.circle_around_session = None;
        let session = self.polygon_session.as_mut().unwrap();
        session.input.set_guide(frame);
        session.released_guide = release;
        self.polygon_prompt();
        true
    }

    fn advance_polygon(&mut self, result: Result<Option<Definition>, GeometryError>) -> bool {
        let polyline = match result {
            Ok(None) => {
                self.polygon_prompt();
                return true;
            }
            Ok(Some(definition)) => definition.polyline(self.document.tolerance()),
            Err(error) => Err(error),
        };
        let polyline = match polyline {
            Ok(polyline) => polyline,
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let mut session = self.polygon_session.take().unwrap();
        let result = (|| -> Result<ObjectId, DocumentError> {
            self.document.begin_group_transaction(&session.group)?;
            let result = self
                .document
                .release_command_selection_on_history_replay(session.released_guide)
                .and_then(|()| self.document.add_geometry(Geometry::Polyline(polyline)));
            let id = match result {
                Ok(id) => id,
                Err(error) => {
                    self.document.rollback_transaction()?;
                    return Err(error);
                }
            };
            self.document.commit_group_transaction(&mut session.group)?;
            Ok(id)
        })();
        self.cancel_interactive_command(false);
        match result {
            Ok(id) => {
                self.push_log(format!("Added polygon {id}"));
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
