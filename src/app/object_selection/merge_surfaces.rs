//! MergeSrf options stay with selection; the second accepted pick executes.
use super::*;
use viboceros_command::merge_surfaces::{Options, parse};

impl VibocerosApp {
    pub(super) fn initialize_merge_surface_options(&mut self, input: &str) {
        if self
            .object_prompt
            .as_ref()
            .is_none_or(|p| p.description.command != "MergeSrf")
        {
            return;
        }
        let result = self
            .commands
            .merge_surface_options()
            .for_document(self.document.tolerance())
            .and_then(|base| {
                parse(
                    &input.split_whitespace().skip(1).collect::<Vec<_>>(),
                    base,
                    self.document.units(),
                )
            });
        match result.and_then(|o| {
            o.effective_tolerance(self.document.tolerance())?;
            Ok(o)
        }) {
            Ok(options) => self.store_merge_surface_options(options),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    fn store_merge_surface_options(&mut self, options: Options) {
        if let Err(error) = self.commands.remember_merge_surface_options(options) {
            self.push_log(format!("Error: {error}"));
            return;
        }
        let t = options
            .effective_tolerance(self.document.tolerance())
            .unwrap();
        let pending = self.object_prompt.as_mut().unwrap();
        pending.description.options[0].value = options.smooth;
        // Smooth also has a generic UI toggle. Keep it in command-owned
        // preferences so a toggle cannot leave an old override in the input.
        pending.command_override = Some(format!(
            "MergeSrf Tolerance={t} Roundness={}",
            options.roundness
        ));
    }

    pub(super) fn continue_merge_surface_options(&mut self, input: &str) -> bool {
        let Some(pending) = self
            .object_prompt
            .as_ref()
            .filter(|p| p.description.command == "MergeSrf")
        else {
            return false;
        };
        let text = input.trim();
        if text.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            return false;
        }
        let field = match pending.phase {
            ObjectPromptPhase::MergeSurfaceValue(name) => Some(name),
            _ => None,
        };
        if text.is_empty() {
            if field.is_some() {
                self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Selecting;
                self.command_input.clear();
                self.log_object_prompt();
                return true;
            }
            if self
                .document
                .selected_objects()
                .filter(|o| ObjectSelectionFilter::Surfaces.accepts_object(o))
                .count()
                != 2
            {
                self.push_log(
                    "Select a pair of surfaces; the second pick merges, Esc cancels".into(),
                );
                self.command_input.clear();
                return true;
            }
            return false;
        }
        let name = text.trim_start_matches('_');
        if field.is_none()
            && ["Tolerance", "Roundness"]
                .iter()
                .any(|n| name.eq_ignore_ascii_case(n))
        {
            let field = if name.eq_ignore_ascii_case("Tolerance") {
                "Tolerance"
            } else {
                "Roundness"
            };
            self.object_prompt.as_mut().unwrap().phase =
                ObjectPromptPhase::MergeSurfaceValue(field);
            let options = self.commands.merge_surface_options();
            let value = if field == "Tolerance" {
                options
                    .effective_tolerance(self.document.tolerance())
                    .unwrap()
            } else {
                options.roundness
            };
            self.push_log(format!(
                "{field} <{value}>; Enter keeps it, Esc cancels MergeSrf"
            ));
            self.command_input.clear();
            return true;
        }
        let options = self.commands.merge_surface_options();
        let arguments = if let Some(field) = field {
            if text
                .split_whitespace()
                .next()
                .is_some_and(|n| self.commands.recognizes(n))
            {
                return false;
            }
            format!("{field}={text}")
        } else if name.eq_ignore_ascii_case("Smooth") {
            format!("Smooth={}", if options.smooth { "No" } else { "Yes" })
        } else if text.split_whitespace().next().is_some_and(|token| {
            let name = token.split('=').next().unwrap().trim_start_matches('_');
            ["Smooth", "Tolerance", "Roundness"]
                .iter()
                .any(|n| name.eq_ignore_ascii_case(n))
        }) {
            text.to_owned()
        } else {
            return false;
        };
        let result = parse(
            &arguments.split_whitespace().collect::<Vec<_>>(),
            options,
            self.document.units(),
        )
        .and_then(|o| {
            o.effective_tolerance(self.document.tolerance())?;
            Ok(o)
        });
        match result {
            Ok(options) => {
                self.store_merge_surface_options(options);
                self.object_prompt.as_mut().unwrap().phase = ObjectPromptPhase::Selecting;
                self.push_log(format!(
                    "MergeSrf {}",
                    options.arguments(self.document.tolerance()).unwrap()
                ));
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }
}
