//! Arc command option continuation outside the application dispatcher.
use super::*;

impl VibocerosApp {
    pub(super) fn try_continue_arc(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        if matches!(
            self.active_command,
            Some(InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                ..
            })
        ) && input
            .trim()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Extension")
        {
            self.start_arc_extension(true);
            self.command_input.clear();
            return true;
        }
        if let Some(InteractiveCommand::ArcCenter {
            center: Some(center),
            start: None,
            ..
        }) = self.active_command
            && input
                .trim()
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Tilted")
        {
            self.start_tilted_arc_from_prompt(center);
            self.command_input.clear();
            return true;
        }
        if matches!(
            self.active_command,
            Some(InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                ..
            })
        ) && input
            .trim()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Tangent")
        {
            self.start_arc_tangent_from_prompt();
            self.command_input.clear();
            return true;
        }
        if matches!(
            self.active_command,
            Some(InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                ..
            })
        ) && input
            .trim()
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Center")
        {
            let command = InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                clockwise: true,
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        if matches!(
            self.active_command,
            Some(InteractiveCommand::ArcCenter {
                center: None,
                start: None,
                ..
            })
        ) && input
            .trim()
            .trim_start_matches('_')
            .eq_ignore_ascii_case("StartPoint")
        {
            let command = InteractiveCommand::ArcStartPoint {
                start: None,
                end: None,
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        if let Some(InteractiveCommand::ArcStartPoint {
            start: Some(start),
            end: None,
        }) = self.active_command
            && input
                .trim()
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Direction")
        {
            let command = InteractiveCommand::ArcStartDirection {
                start: Some(start),
                tangent: None,
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        if let Some(InteractiveCommand::ArcStartPoint {
            start: Some(start),
            end: None,
        }) = self.active_command
            && input
                .trim()
                .trim_start_matches('_')
                .eq_ignore_ascii_case("ThroughPoint")
        {
            let command = InteractiveCommand::Arc {
                points: [Some(start), None],
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        if let Some(InteractiveCommand::ArcStartPoint {
            start: Some(start),
            end: None,
        }) = self.active_command
            && input
                .trim()
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Center")
        {
            let command = InteractiveCommand::ArcStartCenter {
                start: Some(start),
                center: None,
                clockwise: true,
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        if let Some(InteractiveCommand::ArcCenter {
            center: Some(center),
            start: None,
            ..
        }) = self.active_command
            && input
                .trim()
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Midpoint")
        {
            let command = InteractiveCommand::ArcCenterMidpoint {
                center: Some(center),
                midpoint: None,
                clockwise: false,
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        let (center, start, is_length, start_first, midpoint_mode) = match self.active_command {
            Some(InteractiveCommand::ArcCenter {
                center: Some(center),
                start: Some(start),
                ..
            }) => (center, start, false, false, false),
            Some(InteractiveCommand::ArcCenterLength { center, start }) => {
                (center, start, true, false, false)
            }
            Some(InteractiveCommand::ArcStartCenter {
                start: Some(start),
                center: Some(center),
                ..
            }) => (center, start, false, true, false),
            Some(InteractiveCommand::ArcStartCenterLength { start, center }) => {
                (center, start, true, true, false)
            }
            Some(InteractiveCommand::ArcCenterMidpoint {
                center: Some(center),
                midpoint: Some(midpoint),
                ..
            }) => (center, midpoint, false, false, true),
            Some(InteractiveCommand::ArcCenterMidpointLength { center, midpoint }) => {
                (center, midpoint, true, false, true)
            }
            _ => return false,
        };
        let input = input.trim();
        let direction_value = input
            .split_once('=')
            .or_else(|| input.split_once(' '))
            .filter(|(name, _)| {
                name.trim_start_matches('_')
                    .eq_ignore_ascii_case("Direction")
            })
            .map(|(_, value)| value.trim().trim_start_matches('_'));
        if let Some(direction) = direction_value {
            let clockwise = if direction.eq_ignore_ascii_case("Clockwise")
                || direction.eq_ignore_ascii_case("CW")
            {
                true
            } else if direction.eq_ignore_ascii_case("Counterclockwise")
                || direction.eq_ignore_ascii_case("CCW")
            {
                false
            } else {
                self.push_log("Error: direction must be Clockwise or Counterclockwise".into());
                return true;
            };
            if is_length {
                if let Some(s) = self.arc_length_session.as_mut() {
                    s.clockwise = clockwise;
                }
                self.command_input.clear();
                return true;
            }
            let command = if midpoint_mode {
                InteractiveCommand::ArcCenterMidpoint {
                    center: Some(center),
                    midpoint: Some(start),
                    clockwise,
                }
            } else if start_first {
                InteractiveCommand::ArcStartCenter {
                    start: Some(start),
                    center: Some(center),
                    clockwise,
                }
            } else {
                InteractiveCommand::ArcCenter {
                    center: Some(center),
                    start: Some(start),
                    clockwise,
                }
            };
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        if !is_length && input.trim_start_matches('_').eq_ignore_ascii_case("Length") {
            let command = if midpoint_mode {
                InteractiveCommand::ArcCenterMidpointLength {
                    center,
                    midpoint: start,
                }
            } else if start_first {
                InteractiveCommand::ArcStartCenterLength { start, center }
            } else {
                InteractiveCommand::ArcCenterLength { center, start }
            };
            self.start_arc_length();
            self.active_command = Some(command);
            self.command_input.clear();
            self.push_log(command.prompt().to_owned());
            return true;
        }
        let length_value = input
            .split_once('=')
            .filter(|(name, _)| name.trim_start_matches('_').eq_ignore_ascii_case("Length"))
            .map(|(_, value)| value)
            .or_else(|| {
                input
                    .split_once(' ')
                    .filter(|(name, _)| name.trim_start_matches('_').eq_ignore_ascii_case("Length"))
                    .map(|(_, value)| value)
            });
        let is_length = is_length || length_value.is_some();
        let Ok(value) = length_value.unwrap_or(input).trim().parse::<f64>() else {
            return false;
        };
        self.command_input.clear();
        let valid = value.is_finite() && value != 0.0;
        if !valid {
            self.push_log(if is_length {
                "Error: arc length must be a nonzero finite number".into()
            } else {
                "Error: arc angle must be a nonzero finite number".into()
            });
            return true;
        }
        let value = if is_length && self.arc_length_session.is_some_and(|s| s.clockwise) {
            -value
        } else {
            value
        };
        let size = if is_length {
            format!("Length={value}")
        } else {
            value.to_string()
        };
        self.execute_arc_command(&if midpoint_mode {
            format!(
                "Arc Center {} Midpoint {} {size}",
                format_model_point(center),
                format_model_point(start)
            )
        } else if start_first {
            format!(
                "Arc StartPoint {} Center={} {size}",
                format_model_point(start),
                format_model_point(center)
            )
        } else {
            format!(
                "Arc Center {} {} {size}",
                format_model_point(center),
                format_model_point(start)
            )
        });
        true
    }
}
