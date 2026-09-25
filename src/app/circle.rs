//! Interactive Circle point and size options.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CircleSizeMode {
    Radius,
    Diameter,
    Circumference,
    Area,
}

impl CircleSizeMode {
    fn parse(name: &str) -> Option<Self> {
        let name = name.trim_start_matches(['_', '-']);
        if name.eq_ignore_ascii_case("Radius") {
            Some(Self::Radius)
        } else if name.eq_ignore_ascii_case("Diameter") {
            Some(Self::Diameter)
        } else if name.eq_ignore_ascii_case("Circumference") {
            Some(Self::Circumference)
        } else if name.eq_ignore_ascii_case("Area") {
            Some(Self::Area)
        } else {
            None
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Radius => "Radius",
            Self::Diameter => "Diameter",
            Self::Circumference => "Circumference",
            Self::Area => "Area",
        }
    }

    fn radius(self, value: f64) -> f64 {
        match self {
            Self::Radius => value,
            Self::Diameter => value * 0.5,
            Self::Circumference => value / std::f64::consts::TAU,
            Self::Area => (value / std::f64::consts::PI).sqrt(),
        }
    }

    pub(super) const fn prompt(self) -> &'static str {
        match self {
            Self::Radius => "Circle Radius: pick a radius point or enter a radius (Esc cancels)",
            Self::Diameter => {
                "Circle Diameter: pick a radius point or enter a diameter (Esc cancels)"
            }
            Self::Circumference => {
                "Circle Circumference: pick a size point or enter a circumference (Esc cancels)"
            }
            Self::Area => "Circle Area: pick a size point or enter an area (Esc cancels)",
        }
    }
}

impl VibocerosApp {
    pub(super) fn try_continue_circle(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        let Some(state) = self.active_command else {
            return false;
        };
        if state == (InteractiveCommand::Circle { center: None }) {
            let option = input.trim().trim_start_matches('_');
            let next = if option.eq_ignore_ascii_case("2Point") {
                InteractiveCommand::CircleTwoPoint { first: None }
            } else if option.eq_ignore_ascii_case("3Point") {
                InteractiveCommand::CircleThreePoint { points: [None; 2] }
            } else if option.eq_ignore_ascii_case("Vertical") {
                InteractiveCommand::CircleVertical {
                    center: None,
                    radius: None,
                    mode: CircleSizeMode::Radius,
                }
            } else {
                return false;
            };
            self.command_input.clear();
            self.active_command = Some(next);
            self.push_log(next.prompt().to_owned());
            return true;
        }
        if let InteractiveCommand::CircleVertical {
            center: Some(center),
            radius,
            mode,
        } = state
        {
            let input = input.trim();
            let (next_mode, value) = if let Some((name, value)) = input.split_once('=') {
                let Some(next_mode) = CircleSizeMode::parse(name) else {
                    return false;
                };
                (next_mode, Some(value.trim()))
            } else if let Some(next_mode) = CircleSizeMode::parse(input) {
                (next_mode, None)
            } else if let Some((name, value)) = input.split_once(char::is_whitespace) {
                let Some(next_mode) = CircleSizeMode::parse(name) else {
                    return false;
                };
                (next_mode, Some(value.trim()))
            } else if input.parse::<f64>().is_ok() {
                (mode, Some(input))
            } else {
                return false;
            };
            let next_radius = if let Some(value) = value {
                let Ok(value) = value.parse::<f64>() else {
                    self.push_log("Error: circle size must be a finite number".into());
                    self.command_input.clear();
                    return true;
                };
                let converted = next_mode.radius(value);
                if !converted.is_finite() || converted <= self.document.tolerance().absolute() {
                    self.push_log("Error: circle radius must exceed document tolerance".into());
                    self.command_input.clear();
                    return true;
                }
                Some(converted)
            } else if next_mode == mode {
                radius
            } else {
                None
            };
            let next = InteractiveCommand::CircleVertical {
                center: Some(center),
                radius: next_radius,
                mode: next_mode,
            };
            self.active_command = Some(next);
            self.command_input.clear();
            self.push_log(next.prompt().to_owned());
            return true;
        }
        let (center, current_mode) = match state {
            InteractiveCommand::Circle {
                center: Some(center),
            } => (center, CircleSizeMode::Radius),
            InteractiveCommand::CircleSize { center, mode } => (center, mode),
            _ => return false,
        };
        let input = input.trim();
        let (mode, value) = if let Some((name, value)) = input.split_once('=') {
            let Some(mode) = CircleSizeMode::parse(name) else {
                return false;
            };
            (mode, Some(value.trim()))
        } else {
            let mut parts = input.split_whitespace();
            match (parts.next(), parts.next(), parts.next()) {
                (Some(name), None, None) if CircleSizeMode::parse(name).is_some() => {
                    (CircleSizeMode::parse(name).unwrap(), None)
                }
                (Some(name), Some(value), None) if CircleSizeMode::parse(name).is_some() => {
                    (CircleSizeMode::parse(name).unwrap(), Some(value))
                }
                (Some(value), None, None) if value.parse::<f64>().is_ok() => {
                    (current_mode, Some(value))
                }
                _ => return false,
            }
        };
        self.command_input.clear();
        if let Some(value) = value {
            let next = InteractiveCommand::CircleSize { center, mode };
            self.finish_circle_size(next, &format!("{}={value}", mode.label()));
        } else {
            let next = InteractiveCommand::CircleSize { center, mode };
            self.active_command = Some(next);
            self.push_log(next.prompt().to_owned());
        }
        true
    }

    pub(super) fn finish_circle_size(&mut self, state: InteractiveCommand, argument: &str) -> bool {
        let center = match state {
            InteractiveCommand::Circle {
                center: Some(center),
            }
            | InteractiveCommand::CircleSize { center, .. } => center,
            _ => unreachable!("circle size requires a center"),
        };
        let command = format!("Circle {} {argument}", format_model_point(center));
        let drafting_plane = self.drafting_plane;
        self.active_command = None;
        self.command_input.clear();
        let success = self.try_execute_command(&command);
        if !success {
            self.active_command = Some(state);
            self.drafting_plane = drafting_plane;
            self.push_log(state.prompt().to_owned());
        }
        success
    }

    pub(super) fn finish_circle_vertical(
        &mut self,
        state: InteractiveCommand,
        point: Point3,
    ) -> bool {
        let InteractiveCommand::CircleVertical {
            center: Some(center),
            radius,
            ..
        } = state
        else {
            unreachable!("vertical circle requires a center");
        };
        let command = if let Some(radius) = radius {
            format!(
                "Circle Vertical {} {radius} {}",
                format_model_point(center),
                format_model_point(point)
            )
        } else {
            format!(
                "Circle Vertical {} {}",
                format_model_point(center),
                format_model_point(point)
            )
        };
        let drafting_plane = self.drafting_plane;
        self.active_command = None;
        self.command_input.clear();
        let success = self.try_execute_command(&command);
        if !success {
            self.active_command = Some(state);
            self.drafting_plane = drafting_plane;
            self.push_log(state.prompt().to_owned());
        }
        success
    }
}
