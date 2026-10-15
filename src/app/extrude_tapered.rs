use super::*;
use viboceros_command::extrude_tapered::TaperedExtrusionOptions;

impl VibocerosApp {
    pub(super) fn finish_tapered_extrusion(
        &mut self,
        distance: &str,
        options: TaperedExtrusionOptions,
    ) -> bool {
        if matches!(
            self.active_command,
            Some(InteractiveCommand::ExtrudeSurfaceTapered { .. })
        ) {
            return self.finish_surface_face_command(&format!(
                "ExtrudeSrfTapered {distance} {}",
                options.command_options()
            ));
        }
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
        let (surface, mut base, mut options) = match self.active_command {
            Some(InteractiveCommand::ExtrudeCurveTapered { base, options }) => {
                (false, base, options)
            }
            Some(InteractiveCommand::ExtrudeSurfaceTapered { base, options }) => {
                (true, base, options)
            }
            _ => return false,
        };
        let interactive = |base, options| {
            if surface {
                InteractiveCommand::ExtrudeSurfaceTapered { base, options }
            } else {
                InteractiveCommand::ExtrudeCurveTapered { base, options }
            }
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
                    self.active_command = Some(interactive(base, options));
                    self.push_log(options.command_options());
                }
                Err(error) => {
                    let error = match error {
                        viboceros_command::CommandError::Usage(_) if surface => {
                            viboceros_command::CommandError::Usage(
                                viboceros_command::extrude_surface_tapered::USAGE,
                            )
                        }
                        e => e,
                    };
                    self.push_log(format!("Error: {error}"));
                }
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
            self.active_command = Some(interactive(base, options));
            self.command_input.clear();
            self.push_log(format!(
                "{}: pick two direction points",
                if surface {
                    "ExtrudeSrfTapered"
                } else {
                    "ExtrudeCrvTapered"
                }
            ));
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
