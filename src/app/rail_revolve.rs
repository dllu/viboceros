//! Axis picking and command options for rail-guided revolution.
use super::*;
use viboceros_command::rail_revolve::RailRevolveOptions;

impl VibocerosApp {
    pub(super) fn continue_rail_revolve_input(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::RailRevolve(mut state)) = self.active_command else {
            return false;
        };
        if input.is_empty() {
            if let Some(start) = state.axis_start {
                let direction = self.viewports[self.active_viewport]
                    .construction_plane()
                    .z_axis()
                    .as_vector();
                match start.translated(direction) {
                    Ok(end) => {
                        self.apply_drafting_point(end);
                    }
                    Err(error) => self.push_log(format!("Error: {error}")),
                }
                return true;
            }
            return false;
        }
        let key = input
            .split_whitespace()
            .next()
            .unwrap()
            .split('=')
            .next()
            .unwrap()
            .trim_start_matches(['_', '-']);
        if !matches!(
            key.to_ascii_lowercase().as_str(),
            "scaleheight" | "splitattangents"
        ) {
            return false;
        }
        match state.options.update(input) {
            Ok(options) => {
                state.options = options;
                if let Ok(Some(prompt)) = self
                    .commands
                    .object_selection_prompt(&format!("RailRevolve{}", options.arguments()))
                    && let Err(error) = self.commands.accept_object_selection_options(&prompt)
                {
                    self.push_log(format!("Error: {error}"));
                    return true;
                }
                self.active_command = Some(InteractiveCommand::RailRevolve(state));
                self.command_input.clear();
                self.push_log(self.active_command.unwrap().prompt().into());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn rail_revolve_selection_input(
        &self,
        input: &str,
        prompt: &viboceros_command::ObjectSelectionPrompt,
    ) -> Result<String, viboceros_command::CommandError> {
        viboceros_command::rail_revolve::selection_input(
            input,
            RailRevolveOptions::from_prompt(prompt),
        )
    }
}
