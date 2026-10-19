//! Numeric circle sizing follows the shared object-picking and cancellation flow.
use super::*;
use viboceros_command::modify_radius::{self, Input};

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    input: Input,
}
impl VibocerosApp {
    pub(super) fn initialize_modify_radius(&mut self, command: &str) {
        let Some(p) = self
            .object_prompt
            .as_mut()
            .filter(|p| p.description.command == "ModifyRadius")
        else {
            return;
        };
        let words = command.split_whitespace().skip(1).collect::<Vec<_>>();
        if let Ok(mut input) = modify_radius::parse(&words, Input::default()) {
            input.copy =
                Some(input.copy.unwrap_or_else(|| {
                    self.commands.copy_default("ModifyRadius").unwrap_or(false)
                }));
            p.modify_radius = Some(Session { input });
        }
    }
    pub(super) fn continue_modify_radius(&mut self, text: &str) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "ModifyRadius")
        else {
            return false;
        };
        if text
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Cancel")
        {
            return false;
        }
        let first = text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .split('=')
            .next()
            .unwrap_or("");
        if viboceros_command::circle_input::CircleSizeMode::parse(first).is_none()
            && !first.trim_start_matches('_').eq_ignore_ascii_case("Copy")
            && self.commands.recognizes(first)
        {
            return false;
        }
        let Some(session) = &pending.modify_radius else {
            return false;
        };
        let mut input = session.input;
        if pending.phase == ObjectPromptPhase::Selecting {
            if text.is_empty() {
                if input.value.is_none()
                    || !self
                        .document
                        .selected_objects()
                        .any(|o| o.geometry().curve_ref().is_some())
                {
                    return false;
                }
            } else if matches!(
                text.trim_start_matches('_').to_ascii_lowercase().as_str(),
                "selall" | "selnone"
            ) {
                return false;
            }
        }
        if !text.is_empty() {
            input.value = None;
            match modify_radius::parse(&text.split_whitespace().collect::<Vec<_>>(), input) {
                Ok(next) => input = next,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.command_input.clear();
                    return true;
                }
            }
            let pending = self.object_prompt.as_mut().unwrap();
            pending.modify_radius = Some(Session { input });
            pending.description.options[0].value = input.copy.unwrap_or(false);
            if input.value.is_none() || pending.phase == ObjectPromptPhase::Selecting {
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
        }
        let Some(value) = input.value else {
            self.push_log("Enter a positive size; Esc cancels".into());
            self.command_input.clear();
            return true;
        };
        let command = format!(
            "ModifyRadius {} {value} Copy={}",
            input.mode.label(),
            if input.copy.unwrap_or(false) {
                "Yes"
            } else {
                "No"
            }
        );
        let postselected = self.object_prompt.as_ref().unwrap().postselected;
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let result = if postselected {
            self.commands
                .execute_postselected(&mut self.document, &command, context)
        } else {
            self.commands
                .execute_in_context(&mut self.document, &command, context)
        };
        match result {
            Ok(message) => {
                self.object_prompt = None;
                self.push_log(format!("> {command}"));
                self.push_log(message);
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
