//! Typed numeric Rebuild edits share object selection and command preferences.
use super::object_selection::ObjectPromptPhase;
use super::*;
use viboceros_command::surface_rebuild::{self, Options};

const NAMES: [&str; 7] = [
    "UPointCount",
    "VPointCount",
    "UDegree",
    "VDegree",
    "DeleteInput",
    "OutputLayer",
    "ReTrim",
];

impl VibocerosApp {
    pub(super) fn initialize_rebuild_options(&mut self) {
        let Some(p) = self.object_prompt.as_ref().filter(|p| {
            p.description.command == "Rebuild" && p.phase == ObjectPromptPhase::Options
        }) else {
            return;
        };
        let defaults = self.commands.surface_rebuild_defaults();
        let input = p.command_override.as_deref().unwrap_or("Rebuild");
        match surface_rebuild::parse(
            &input.split_whitespace().skip(1).collect::<Vec<_>>(),
            defaults,
        ) {
            Ok(options) => self.store_rebuild_options(options),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn store_rebuild_options(&mut self, options: Options) {
        self.commands.remember_surface_rebuild_options(options);
        let p = self.object_prompt.as_mut().unwrap();
        p.command_override = Some(options.command_line());
        p.phase = ObjectPromptPhase::Options;
    }

    pub(super) fn continue_rebuild_options(&mut self, input: &str) -> bool {
        let Some(p) = self.object_prompt.as_ref().filter(|p| {
            p.description.command == "Rebuild" && p.phase != ObjectPromptPhase::Selecting
        }) else {
            return false;
        };
        // Curve Rebuild retains its existing immediate behavior; this prompt
        // belongs to the measured surface branch.
        if !self
            .document
            .selected_objects()
            .any(|o| viboceros_command::ObjectSelectionFilter::Surfaces.accepts_object(o))
        {
            return false;
        }
        let phase = p.phase;
        if input.is_empty() {
            if matches!(phase, ObjectPromptPhase::RebuildValue(_)) {
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
        if normalized.eq_ignore_ascii_case("Cancel") {
            self.cancel_object_prompt(true);
            self.command_input.clear();
            return true;
        }
        let name = normalized.split(['=', ' ']).next().unwrap_or("");
        let local = NAMES.iter().any(|n| n.eq_ignore_ascii_case(name));
        if !local && phase == ObjectPromptPhase::Options && self.commands.recognizes(name) {
            return false;
        }
        if phase == ObjectPromptPhase::Options
            && let Some(&name) = NAMES.iter().find(|n| n.eq_ignore_ascii_case(normalized))
        {
            self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::RebuildValue(name);
            self.push_log(format!(
                "{name}: {}",
                self.commands.surface_rebuild_defaults().command_line()
            ));
            self.command_input.clear();
            return true;
        }
        let update = match phase {
            ObjectPromptPhase::RebuildValue(name) => format!("{name}={normalized}"),
            _ => input.to_owned(),
        };
        match surface_rebuild::parse(
            &update.split_whitespace().collect::<Vec<_>>(),
            self.commands.surface_rebuild_defaults(),
        ) {
            Ok(options) => {
                self.store_rebuild_options(options);
                self.push_log(options.command_line());
                self.command_input.clear();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}
