//! Numeric tolerance input stays outside document history until acceptance.
use super::*;
use viboceros_command::reduce_polyline::{self, Input};
use viboceros_geometry::Real;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) struct Session {
    input: Input,
    default: Real,
}
impl Session {
    fn command(&self) -> String {
        format!(
            "ReducePolyline {} Copy={}",
            self.input.tolerance.unwrap_or(self.default),
            if self.input.copy.unwrap_or(false) {
                "Yes"
            } else {
                "No"
            }
        )
    }
}
impl VibocerosApp {
    pub(super) fn initialize_reduce_polyline(&mut self, command: &str) {
        let default = self
            .commands
            .transform_scalar_default("ReducePolyline")
            .unwrap_or(self.document.tolerance().absolute());
        let copy_default = self
            .commands
            .copy_default("ReducePolyline")
            .unwrap_or(false);
        let Some(p) = self
            .object_prompt
            .as_mut()
            .filter(|p| p.description.command == "ReducePolyline")
        else {
            return;
        };
        let words = command.split_whitespace().skip(1).collect::<Vec<_>>();
        if let Ok(mut input) = reduce_polyline::parse(&words, Input::default()) {
            input.copy = Some(input.copy.unwrap_or(copy_default));
            let session = Session { input, default };
            p.command_override = Some(session.command());
            p.reduce_polyline = Some(session);
        }
    }
    pub(super) fn continue_reduce_polyline(&mut self, text: &str) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "ReducePolyline")
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
        if !["Copy", "Tolerance"]
            .iter()
            .any(|n| first.trim_start_matches('_').eq_ignore_ascii_case(n))
            && self.commands.recognizes(first)
        {
            return false;
        }
        let Some(session) = &pending.reduce_polyline else {
            return false;
        };
        let mut session = session.clone();
        if pending.phase == ObjectPromptPhase::Selecting
            && text.is_empty()
            && session.input.tolerance.is_none()
        {
            return false;
        }
        if !text.is_empty() {
            let arguments = text.split_whitespace().collect::<Vec<_>>();
            match reduce_polyline::parse(&arguments, session.input) {
                Ok(input) => session.input = input,
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    self.command_input.clear();
                    return true;
                }
            }
            let has_value = arguments.iter().any(|v| {
                v.parse::<Real>().is_ok()
                    || v.split_once('=').is_some_and(|(key, _)| {
                        key.trim_start_matches('_')
                            .eq_ignore_ascii_case("Tolerance")
                    })
            });
            let p = self.object_prompt.as_mut().unwrap();
            p.description.options[0].value = session.input.copy.unwrap_or(false);
            p.command_override = Some(session.command());
            p.reduce_polyline = Some(session.clone());
            if !has_value || p.phase == ObjectPromptPhase::Selecting {
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
        }
        if !self.document.selected_objects().any(|o| {
            o.geometry()
                .curve_ref()
                .is_some_and(reduce_polyline::accepts_curve)
        }) {
            return false;
        }
        let command = session.command();
        let post = self.object_prompt.as_ref().unwrap().postselected;
        let context = viboceros_command::CommandContext {
            construction_plane: self.viewports[self.active_viewport].construction_plane(),
        };
        let result = if post {
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
