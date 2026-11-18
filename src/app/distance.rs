//! Measurement-local input revision, separate from document undo/redo.
use super::*;

pub(super) fn start_command(
    arguments: &[&str],
    previous_last: Option<Point3>,
) -> Option<InteractiveCommand> {
    let display_units = match arguments {
        [] => None,
        [option] => {
            let (name, value) = option.split_once('=')?;
            if !name
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Units")
            {
                return None;
            }
            viboceros_command::distance_display_units(value).ok()?
        }
        // Coordinates belong to the complete command's parser, not startup.
        _ => return None,
    };
    Some(InteractiveCommand::Distance {
        start: None,
        previous_last,
        display_units,
    })
}

impl VibocerosApp {
    pub(super) fn finish_distance(
        &mut self,
        start: Point3,
        end: Point3,
        units: Option<&'static str>,
        plane: Frame3,
    ) -> bool {
        let nested = self.nested_measurement_kind() == Some(nested_measurement::Kind::Distance);
        let value = if nested {
            match start.distance_to(end) {
                Ok(value) => Some(value),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            }
        } else {
            None
        };
        let input = format!(
            "Distance {} {}{}",
            format_model_point(start),
            format_model_point(end),
            units
                .filter(|_| !nested)
                .map(|unit| format!(" Units={unit}"))
                .unwrap_or_default()
        );
        match self.commands.execute_in_context(
            &mut self.document,
            &input,
            viboceros_command::CommandContext {
                construction_plane: plane,
            },
        ) {
            Ok(message) => {
                if !nested {
                    self.cancel_interactive_command(false);
                }
                self.push_log(format!("> {input}"));
                self.push_log(message);
                value.is_none_or(|value| self.finish_nested_value(value))
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    pub(super) fn try_continue_distance(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::Distance {
            start,
            previous_last,
            display_units,
        }) = self.active_command
        else {
            return false;
        };
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let input = input.trim().trim_start_matches(['_', '-']);
        let named_units = input
            .split_once('=')
            .filter(|(name, _)| name.eq_ignore_ascii_case("Units"))
            .map(|(_, value)| value);
        if input.eq_ignore_ascii_case("Units") {
            self.distance_units_prompt = true;
            self.push_log("Distance units: enter a unit name or Model_Units".into());
            self.command_input.clear();
            return true;
        }
        if self.distance_units_prompt && (input.is_empty() || input.eq_ignore_ascii_case("Enter")) {
            self.distance_units_prompt = false;
            self.push_log(self.active_command.unwrap().prompt().to_owned());
            self.command_input.clear();
            return true;
        }
        if let Some(value) = named_units.or(self.distance_units_prompt.then_some(input)) {
            let nested = self.nested_measurement_kind() == Some(nested_measurement::Kind::Distance);
            let units = if nested {
                viboceros_command::radius_display_units(value)
            } else {
                viboceros_command::distance_display_units(value)
            };
            match units {
                Ok(display_units) => {
                    self.distance_units_prompt = false;
                    if nested {
                        self.nested_distance_units = display_units;
                    }
                    self.active_command = Some(InteractiveCommand::Distance {
                        start,
                        previous_last,
                        display_units,
                    });
                    self.push_log(format!(
                        "Distance display units: {}",
                        display_units.unwrap_or("Model_Units")
                    ));
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
            self.command_input.clear();
            return true;
        }
        if start.is_none() || !input.eq_ignore_ascii_case("Undo") {
            return false;
        }
        let next = InteractiveCommand::Distance {
            start: None,
            previous_last,
            display_units,
        };
        self.active_command = Some(next);
        self.last_point = previous_last;
        self.drafting_plane = None;
        self.command_input.clear();
        self.push_log(next.prompt().to_owned());
        true
    }
}
