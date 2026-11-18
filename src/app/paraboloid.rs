//! Paraboloid point collection and atomic insertion.
use super::*;
use viboceros_command::paraboloid::{Definition, Input};
use viboceros_document::HistoryGroup;
use viboceros_geometry::Brep;

pub(super) struct Session {
    pub(super) input: Input,
    group: HistoryGroup,
}

impl VibocerosApp {
    pub(super) fn try_start_paraboloid(&mut self, text: &str) -> bool {
        let mut words = text.split_whitespace();
        if !words.next().is_some_and(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Paraboloid")
        }) {
            return false;
        }
        let mut input = Input::new(
            self.commands.paraboloid_options(),
            self.document.tolerance(),
        );
        if !words.all(|word| input.option(word).is_ok_and(|handled| handled)) {
            return false;
        }
        self.cancel_interactive_command(false);
        let group = match self.document.begin_history_group("Paraboloid") {
            Ok(group) => group,
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.paraboloid_session = Some(Session { input, group });
        self.commands
            .remember_paraboloid_options(input.preferences());
        self.paraboloid_prompt(input);
        true
    }

    fn paraboloid_prompt(&mut self, input: Input) {
        self.active_command = Some(InteractiveCommand::Paraboloid {
            anchor: input.anchor(),
        });
        self.command_input.clear();
        self.push_log(input.prompt());
    }

    fn paraboloid_is_current(&mut self) -> bool {
        let Some(session) = &self.paraboloid_session else {
            return false;
        };
        if self.document.history_group_is_current(&session.group)
            && session.input.tolerance() == self.document.tolerance()
        {
            return true;
        }
        self.cancel_interactive_command(false);
        self.command_file.fail_dispatch();
        self.push_log("Paraboloid inputs or history changed. Restart the command.".into());
        false
    }

    pub(super) fn try_continue_paraboloid(&mut self, text: &str) -> bool {
        if self.paraboloid_session.is_none() {
            return false;
        }
        let word = text.trim_start_matches(['_', '-']);
        if word.is_empty()
            || word.eq_ignore_ascii_case("Enter")
            || word.eq_ignore_ascii_case("Cancel")
        {
            self.cancel_interactive_command(true);
            self.command_input.clear();
            return true;
        }
        if !self.paraboloid_is_current() {
            return false;
        }
        let input = &mut self.paraboloid_session.as_mut().unwrap().input;
        match input.option(text) {
            Ok(false) => false,
            Ok(true) => {
                let input = *input;
                self.commands
                    .remember_paraboloid_options(input.preferences());
                self.paraboloid_prompt(input);
                true
            }
            Err(error) => {
                self.command_file.fail_dispatch();
                self.command_input.clear();
                self.push_log(format!("Error: {error}"));
                true
            }
        }
    }

    pub(super) fn accept_paraboloid_point(&mut self, point: Point3) -> Option<bool> {
        self.paraboloid_session.as_ref()?;
        if !self.paraboloid_is_current() {
            return Some(false);
        }
        let input = &mut self.paraboloid_session.as_mut().unwrap().input;
        Some(match input.point(point) {
            Ok(None) => {
                let input = *input;
                self.paraboloid_prompt(input);
                true
            }
            Ok(Some(definition)) => match definition.brep() {
                Ok(brep) => self.finish_paraboloid(definition, brep),
                Err(error) => {
                    self.command_file.fail_dispatch();
                    self.push_log(format!("Error: {error}"));
                    false
                }
            },
            Err(error) => {
                self.command_file.fail_dispatch();
                self.push_log(format!("Error: {error}"));
                false
            }
        })
    }

    fn finish_paraboloid(&mut self, definition: Definition, brep: Brep) -> bool {
        let mut session = self.paraboloid_session.take().unwrap();
        let result = (|| -> Result<(), DocumentError> {
            self.document.begin_group_transaction(&session.group)?;
            let add = self
                .document
                .add_geometry(Geometry::Brep(brep))
                .and_then(|_| {
                    if let Some(focus) = definition.focus_marker() {
                        self.document.add_geometry(Geometry::Point(focus))?;
                    }
                    Ok(())
                });
            if let Err(error) = add {
                self.document.rollback_transaction()?;
                return Err(error);
            }
            self.document.commit_group_transaction(&mut session.group)?;
            Ok(())
        })();
        self.cancel_interactive_command(false);
        match result {
            Ok(()) => {
                self.push_log(format!(
                    "Created Paraboloid. Focus to vertex distance = {:.6}",
                    definition.focal_distance()
                ));
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
