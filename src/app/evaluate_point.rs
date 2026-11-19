//! Coordinate queries keep failed point input open for correction.
use super::*;
use viboceros_command::EvaluatePointOptions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OptionName {
    CoordinateSystem,
    Style,
}

impl OptionName {
    fn name(self) -> &'static str {
        match self {
            Self::CoordinateSystem => "CoordinateSystem",
            Self::Style => "Style",
        }
    }
}

pub(super) fn start_command(arguments: &[&str]) -> Option<InteractiveCommand> {
    Some(InteractiveCommand::EvaluatePoint {
        options: EvaluatePointOptions::parse(arguments).ok()?,
        pending: None,
    })
}

impl VibocerosApp {
    pub(super) fn try_continue_evaluate_point(&mut self, input: &str) -> bool {
        let Some(InteractiveCommand::EvaluatePoint {
            mut options,
            pending,
        }) = self.active_command
        else {
            return false;
        };
        let input = input.trim();
        let name = input.trim_start_matches(['_', '-']);
        if name.is_empty() || name.eq_ignore_ascii_case("Cancel") {
            return false;
        }
        let (key, value) = input
            .split_once(|ch: char| ch == '=' || ch.is_whitespace())
            .map_or((input, None), |(key, value)| (key, Some(value.trim())));
        let key = key.trim_start_matches(['_', '-']);
        let next = if key.eq_ignore_ascii_case("Label")
            || key.eq_ignore_ascii_case("CoordinateSystem")
            || key.eq_ignore_ascii_case("Style")
        {
            if let Some(value) = value {
                options.with_option(&format!("{key}={value}")).map(|new| {
                    options = new;
                    None
                })
            } else if key.eq_ignore_ascii_case("Label") {
                options.label = !options.label;
                Ok(None)
            } else if key.eq_ignore_ascii_case("CoordinateSystem") {
                Ok(Some(OptionName::CoordinateSystem))
            } else {
                Ok(Some(OptionName::Style))
            }
        } else if let Some(pending) = pending {
            options
                .with_option(&format!("{}={input}", pending.name()))
                .map(|new| {
                    options = new;
                    None
                })
        } else {
            return false;
        };
        match next {
            Ok(pending) => {
                let command = InteractiveCommand::EvaluatePoint { options, pending };
                self.active_command = Some(command);
                self.push_log(format!("> {input}"));
                self.push_log(command.prompt().to_owned());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    pub(super) fn finish_evaluate_point(
        &mut self,
        point: Point3,
        plane: Frame3,
        options: EvaluatePointOptions,
    ) -> bool {
        let mut input = format!("EvaluatePt {}", format_model_point(point));
        if options.label {
            input.push_str(" Label=On CoordinateSystem=");
            input.push_str(options.coordinate_system());
        }
        match self.commands.execute_in_context(
            &mut self.document,
            &input,
            viboceros_command::CommandContext {
                construction_plane: plane,
            },
        ) {
            Ok(report) => {
                self.cancel_interactive_command(false);
                self.push_log(format!("> {input}"));
                self.push_log(report);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }
}
