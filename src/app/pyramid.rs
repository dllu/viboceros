//! App routing for Pyramid's staged polygon, guide and apex getters.
use super::*;
use viboceros_command::{
    CommandError,
    pyramid::{Definition, Input},
};
use viboceros_document::HistoryGroup;

pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
    released_guide: Option<ObjectId>,
}

impl VibocerosApp {
    pub(super) fn try_start_pyramid(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|name| {
            name.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Pyramid")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.viewports[self.active_viewport].construction_plane(),
            self.commands.pyramid_options(),
            self.document.tolerance(),
        );
        if !words.all(|word| input.option(word).is_ok_and(|handled| handled)) {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("Pyramid") {
            Ok(group) => group,
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_file.fail_dispatch();
                return true;
            }
        };
        self.pyramid_session = Some(Session {
            input,
            group,
            released_guide: None,
        });
        self.pyramid_prompt();
        true
    }

    pub(super) fn valid_pyramid(&mut self) -> bool {
        if let Some(session) = &self.pyramid_session
            && (!self.document.history_group_is_current(&session.group)
                || self.document.tolerance() != session.input.tolerance())
        {
            self.cancel_interactive_command(false);
            self.push_log("Pyramid inputs or history changed. Restart the command.".to_owned());
            self.command_file.fail_dispatch();
            return false;
        }
        true
    }

    fn pyramid_prompt(&mut self) {
        let input = self.pyramid_session.as_ref().unwrap().input;
        self.commands.remember_pyramid_options(input.preferences());
        self.point_filter = None;
        self.point_constraint = None;
        self.command_input.clear();
        if input.needs_guide() && self.circle_around_session.is_none() {
            self.start_pyramid_around();
            return;
        }
        self.active_command = Some(InteractiveCommand::Pyramid {
            anchor: input.anchor(),
        });
        self.push_log(input.prompt());
    }

    pub(super) fn try_edit_pyramid_guide(&mut self, text: &str) -> bool {
        let input = &mut self.pyramid_session.as_mut().unwrap().input;
        match input.option(text) {
            Ok(false) => false,
            Ok(true) => {
                if !input.needs_guide() {
                    self.release_circle_around_picks();
                    self.circle_around_session = None;
                }
                self.pyramid_prompt();
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                self.command_input.clear();
                true
            }
        }
    }

    fn refresh_pyramid_star_roll(&mut self) {
        let input = &mut self.pyramid_session.as_mut().unwrap().input;
        if let Some(plane) = input.star_roll_plane()
            && let Some(ray) = self.viewports[self.active_viewport].pointer_ray()
            && let Ok(Some(point)) = ray.on_plane(plane)
        {
            input.observe_cursor(point);
        }
    }

    pub(super) fn try_continue_pyramid(&mut self, text: &str) -> bool {
        if self.pyramid_session.is_none() || self.circle_around_session.is_some() {
            return false;
        }
        if !self.valid_pyramid() {
            return false;
        }
        let word = text.trim().trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        self.refresh_pyramid_star_roll();
        let input = &mut self.pyramid_session.as_mut().unwrap().input;
        let result = match input.option(text) {
            Ok(true) => {
                self.pyramid_prompt();
                return true;
            }
            Err(error) => Err(error),
            Ok(false) if word.is_empty() || word.eq_ignore_ascii_case("Enter") => input.enter(),
            Ok(false) if input.wants_number() => {
                let Some(value) = viboceros_drafting::PointInput::parse_length_with_units(
                    text,
                    self.document.units(),
                ) else {
                    return false;
                };
                value
                    .map_err(|_| CommandError::InvalidNumber(text.to_owned()))
                    .and_then(|v| input.number(v))
            }
            Ok(false) => return false,
        };
        self.command_input.clear();
        self.advance_pyramid(result);
        true
    }

    pub(super) fn accept_pyramid_point(&mut self, point: Point3) -> Option<bool> {
        self.pyramid_session.as_ref()?;
        if !self.valid_pyramid() {
            return Some(false);
        }
        self.refresh_pyramid_star_roll();
        let input = &mut self.pyramid_session.as_mut().unwrap().input;
        input.set_initial_plane(self.viewports[self.active_viewport].construction_plane());
        let result = input.point(point);
        Some(self.advance_pyramid(result))
    }

    pub(super) fn accept_pyramid_guide(
        &mut self,
        frame: Frame3,
        release: Option<ObjectId>,
    ) -> bool {
        let session = self.pyramid_session.as_mut().unwrap();
        let result = session.input.set_guide(frame);
        if result.is_ok() {
            session.released_guide = release;
            self.release_circle_around_picks();
            self.circle_around_session = None;
        }
        self.advance_pyramid(result.map(|()| None))
    }

    fn advance_pyramid(&mut self, result: Result<Option<Definition>, CommandError>) -> bool {
        let definition = match result {
            Ok(Some(definition)) => definition,
            Ok(None) => {
                self.pyramid_prompt();
                return true;
            }
            Err(CommandError::OperationDeclined) => {
                self.cancel_interactive_command(true);
                return false;
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let brep = match definition.brep(self.document.tolerance()) {
            Ok(brep) => brep,
            Err(error) => {
                let old = self.commands.pyramid_options();
                self.pyramid_session
                    .as_mut()
                    .unwrap()
                    .input
                    .restore_height(old.height);
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return false;
            }
        };
        let mut session = self.pyramid_session.take().unwrap();
        let result = (|| -> Result<(), DocumentError> {
            self.document.begin_group_transaction(&session.group)?;
            let result = self
                .document
                .release_command_selection_on_history_replay(session.released_guide)
                .and_then(|()| self.document.add_geometry(Geometry::Brep(brep)));
            if let Err(error) = result {
                self.document.rollback_transaction()?;
                return Err(error);
            }
            self.document.commit_group_transaction(&mut session.group)?;
            Ok(())
        })();
        self.cancel_interactive_command(false);
        match result {
            Ok(()) => {
                self.commands
                    .remember_pyramid_options(session.input.preferences());
                self.push_log("Created Pyramid".to_owned());
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
