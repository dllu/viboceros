//! Numeric Smooth options share the existing object/grip selection phases.
use super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_command::smooth::{self, Options};

impl VibocerosApp {
    pub(super) fn initialize_smooth_options(&mut self, input: &str, preselected: bool) {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.description.command != "Smooth")
        {
            return;
        }
        let defaults = self.commands.smooth_options_default();
        let words = input.split_whitespace().skip(1).collect::<Vec<_>>();
        let options = if preselected {
            smooth::parse(&words, defaults).expect("startup arguments already validated")
        } else {
            defaults
        };
        self.store_smooth_prompt(options);
    }

    fn store_smooth_prompt(&mut self, options: Options) {
        let pending = self.object_prompt.as_mut().unwrap();
        pending.description = smooth::prompt(options);
        pending.command_override = Some(options.command_line());
    }

    pub(super) fn continue_smooth_options(&mut self, input: &str) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "Smooth")
        else {
            return false;
        };
        let phase = pending.phase;
        if phase == ObjectPromptPhase::Selecting {
            return false;
        }
        let current = smooth::parse(
            &pending
                .command_override
                .as_ref()
                .unwrap()
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>(),
            self.commands.smooth_options_default(),
        )
        .expect("stored Smooth options");
        if input.is_empty() {
            if matches!(
                phase,
                ObjectPromptPhase::SmoothFactor
                    | ObjectPromptPhase::SmoothSteps
                    | ObjectPromptPhase::Choice(_)
            ) {
                self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Options;
                self.command_input.clear();
                return true;
            }
            return false;
        }
        let normalized = input.trim_start_matches(['_', '-']);
        if normalized.eq_ignore_ascii_case("Enter") {
            return self.try_continue_object_prompt("");
        }
        if normalized.eq_ignore_ascii_case("Cancel") && phase != ObjectPromptPhase::Options {
            self.cancel_object_prompt(true);
            return true;
        }
        // Replacing the command interrupts Smooth without applying it.
        let local_option = phase == ObjectPromptPhase::Options
            && [
                "SmoothFactor",
                "Steps",
                "CoordinateSystem",
                "X",
                "Y",
                "Z",
                "FixBoundaries",
            ]
            .iter()
            .any(|name| {
                normalized
                    .split(['=', ' '])
                    .next()
                    .is_some_and(|token| token.eq_ignore_ascii_case(name))
            });
        if input
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name))
            && !local_option
        {
            return false;
        }
        if phase == ObjectPromptPhase::Options {
            let next = if normalized.eq_ignore_ascii_case("SmoothFactor") {
                Some(ObjectPromptPhase::SmoothFactor)
            } else if normalized.eq_ignore_ascii_case("Steps") {
                Some(ObjectPromptPhase::SmoothSteps)
            } else if normalized.eq_ignore_ascii_case("CoordinateSystem") {
                Some(ObjectPromptPhase::Choice(0))
            } else {
                None
            };
            if let Some(next) = next {
                self.object_prompt.as_mut().unwrap().phase = next;
                self.command_input.clear();
                self.push_log(format!(
                    "{}: {}",
                    self.object_prompt.as_ref().unwrap().label(),
                    current.command_line()
                ));
                return true;
            }
            // Native scripted _Cancel ends the option getter as a successful
            // edit. Keyboard Escape uses the separate interruption path.
            if normalized.eq_ignore_ascii_case("Cancel") {
                return self.try_continue_object_prompt("");
            }
        }
        let update = match phase {
            ObjectPromptPhase::SmoothFactor => {
                format!("SmoothFactor={}", input.trim_start_matches('_'))
            }
            ObjectPromptPhase::SmoothSteps => format!("Steps={}", input.trim_start_matches('_')),
            ObjectPromptPhase::Choice(_) => format!("CoordinateSystem={normalized}"),
            _ => input.to_owned(),
        };
        match smooth::parse(&update.split_whitespace().collect::<Vec<_>>(), current) {
            Ok(options) => {
                self.store_smooth_prompt(options);
                self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Options;
                self.push_log(options.command_line());
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}
