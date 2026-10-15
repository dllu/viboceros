use super::*;
use viboceros_command::extrude_tapered::TaperedExtrusionOptions;

impl VibocerosApp {
    pub(super) fn finish_tapered_extrusion(
        &mut self,
        distance: &str,
        options: TaperedExtrusionOptions,
    ) -> bool {
        let saved = self.active_command;
        let saved_plane = self.drafting_plane;
        let saved_constraint = self.point_constraint.take();
        let saved_filter = self.point_filter.take();
        let command = format!("ExtrudeCrvTapered {distance} {}", options.command_options());
        if self.try_execute_command(&command) {
            self.active_command = None;
            self.drafting_plane = None;
            self.point_constraint = None;
            self.point_filter = None;
            true
        } else {
            self.active_command = saved;
            self.drafting_plane = saved_plane;
            self.point_constraint = saved_constraint;
            self.point_filter = saved_filter;
            false
        }
    }
    pub(super) fn try_continue_tapered_extrusion(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::ExtrudeCurveTapered {
            mut base,
            mut options,
        }) = self.active_command
        else {
            return false;
        };
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return false;
        }
        if words
            .iter()
            .all(|s| s.contains('=') || s.trim_start_matches('_').eq_ignore_ascii_case("FlipAngle"))
        {
            match options.update(&words) {
                Ok(()) => {
                    self.active_command =
                        Some(InteractiveCommand::ExtrudeCurveTapered { base, options });
                    self.push_log(options.command_options());
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        if words.len() == 1
            && matches!(
                words[0]
                    .trim_start_matches('_')
                    .to_ascii_lowercase()
                    .as_str(),
                "direction" | "setbasepoint"
            )
        {
            base = None;
            options.direction = None;
            self.active_command = Some(InteractiveCommand::ExtrudeCurveTapered { base, options });
            self.command_input.clear();
            self.push_log("ExtrudeCrvTapered: pick two direction points".into());
            return true;
        }
        if base.is_none()
            && let Some(length) = viboceros_drafting::PointInput::parse_length_with_units(
                input,
                self.document.units(),
            )
        {
            match length {
                Ok(height) => {
                    self.finish_tapered_extrusion(&height.to_string(), options);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        false
    }
}
