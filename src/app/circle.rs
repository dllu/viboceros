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

fn parse_size_input(
    input: &str,
    current_mode: CircleSizeMode,
) -> Option<(CircleSizeMode, Option<&str>)> {
    let input = input.trim();
    if let Some((name, value)) = input.split_once('=') {
        return Some((CircleSizeMode::parse(name)?, Some(value.trim())));
    }
    let mut parts = input.split_whitespace();
    match (parts.next(), parts.next(), parts.next()) {
        (Some(name), None, None) if CircleSizeMode::parse(name).is_some() => {
            Some((CircleSizeMode::parse(name)?, None))
        }
        (Some(name), Some(value), None) if CircleSizeMode::parse(name).is_some() => {
            Some((CircleSizeMode::parse(name)?, Some(value)))
        }
        (Some(value), None, None) if value.parse::<f64>().is_ok() => {
            Some((current_mode, Some(value)))
        }
        _ => None,
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
        if let InteractiveCommand::CircleThreePoint {
            points: [Some(first), Some(second)],
        } = state
        {
            let option = input.trim().trim_start_matches('_');
            if option.eq_ignore_ascii_case("Radius") {
                let next = InteractiveCommand::CircleThreePointRadius {
                    first,
                    second,
                    radius: None,
                };
                self.active_command = Some(next);
                self.command_input.clear();
                self.push_log(next.prompt().to_owned());
                return true;
            }
            if let Some((name, value)) = option.split_once('=')
                && name.eq_ignore_ascii_case("Radius")
            {
                return self.accept_three_point_radius_value(first, second, value);
            }
        }
        if let InteractiveCommand::CircleThreePointRadius {
            first,
            second,
            radius: None,
        } = state
        {
            if input.trim().parse::<f64>().is_err() {
                return false;
            }
            return self.accept_three_point_radius_value(first, second, input.trim());
        }
        if let InteractiveCommand::CircleVertical {
            center: Some(center),
            radius,
            mode,
        } = state
        {
            let Some((next_mode, value)) = parse_size_input(input, mode) else {
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
        if let InteractiveCommand::CircleOrientation {
            center,
            normal_point: Some(normal_point),
            mode: current_mode,
        } = state
        {
            let Some((mode, value)) = parse_size_input(input, current_mode) else {
                return false;
            };
            let next = InteractiveCommand::CircleOrientation {
                center,
                normal_point: Some(normal_point),
                mode,
            };
            self.command_input.clear();
            if let Some(value) = value {
                self.execute_circle_orientation(next, &format!("{}={value}", mode.label()));
            } else {
                self.active_command = Some(next);
                self.push_log(next.prompt().to_owned());
            }
            return true;
        }
        if input
            .trim()
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Orientation")
        {
            let center = match state {
                InteractiveCommand::Circle {
                    center: Some(center),
                }
                | InteractiveCommand::CircleSize { center, .. } => center,
                _ => return false,
            };
            let next = InteractiveCommand::CircleOrientation {
                center,
                normal_point: None,
                mode: CircleSizeMode::Radius,
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
        let Some((mode, value)) = parse_size_input(input, current_mode) else {
            return false;
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

    fn accept_three_point_radius_value(
        &mut self,
        first: Point3,
        second: Point3,
        value: &str,
    ) -> bool {
        let radius = value.trim().parse::<f64>().ok();
        let chord = first.distance_to(second).ok();
        let valid = radius.zip(chord).is_some_and(|(radius, chord)| {
            radius.is_finite()
                && radius > self.document.tolerance().absolute()
                && radius >= chord * 0.5
        });
        self.command_input.clear();
        if !valid {
            self.push_log("Error: radius must be at least half the point separation".into());
            return true;
        }
        let next = InteractiveCommand::CircleThreePointRadius {
            first,
            second,
            radius,
        };
        self.active_command = Some(next);
        self.push_log(next.prompt().to_owned());
        true
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

    pub(super) fn finish_circle_orientation(
        &mut self,
        state: InteractiveCommand,
        point: Point3,
    ) -> bool {
        let InteractiveCommand::CircleOrientation { center, mode, .. } = state else {
            unreachable!("circle orientation requires a center");
        };
        let argument = if matches!(mode, CircleSizeMode::Radius | CircleSizeMode::Diameter) {
            format_model_point(point)
        } else {
            let Ok(distance) = center.distance_to(point) else {
                self.push_log("Error: circle size is not representable".into());
                return false;
            };
            format!("{}={distance}", mode.label())
        };
        self.execute_circle_orientation(state, &argument)
    }

    fn execute_circle_orientation(&mut self, state: InteractiveCommand, argument: &str) -> bool {
        let InteractiveCommand::CircleOrientation {
            center,
            normal_point: Some(normal_point),
            ..
        } = state
        else {
            unreachable!("circle orientation requires a direction");
        };
        let command = format!(
            "Circle Orientation {} {} {argument}",
            format_model_point(center),
            format_model_point(normal_point)
        );
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

    pub(super) fn finish_three_point_radius(
        &mut self,
        state: InteractiveCommand,
        point: Point3,
    ) -> bool {
        let InteractiveCommand::CircleThreePointRadius {
            first,
            second,
            radius,
        } = state
        else {
            unreachable!("three-point radius requires two circumference points");
        };
        let radius_argument = radius
            .map(|radius| radius.to_string())
            .unwrap_or_else(|| format_model_point(point));
        let command = if radius.is_some() {
            format!(
                "Circle 3Point {} {} Radius={radius_argument} {}",
                format_model_point(first),
                format_model_point(second),
                format_model_point(point)
            )
        } else {
            format!(
                "Circle 3Point {} {} Radius {}",
                format_model_point(first),
                format_model_point(second),
                radius_argument
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
