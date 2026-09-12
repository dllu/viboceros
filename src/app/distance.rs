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
        let input = format!(
            "Distance {} {}{}",
            format_model_point(start),
            format_model_point(end),
            units
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
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(message);
                true
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
        if let Some((name, value)) = input.split_once('=')
            && name.eq_ignore_ascii_case("Units")
        {
            match viboceros_command::distance_display_units(value) {
                Ok(display_units) => {
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
