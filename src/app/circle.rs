//! Interactive Circle point and size options.
use super::*;

use viboceros_command::circle_around_input::Preferences;
pub(super) use viboceros_command::circle_input::CircleSizeMode;
use viboceros_command::circle_input::PointConstruction;
use viboceros_drafting::{PointInput, PointInputError};
use viboceros_geometry::LengthUnitSystem;

fn parse_size_input(
    input: &str,
    current_mode: CircleSizeMode,
    units: &LengthUnitSystem,
    preferences: Preferences,
) -> Option<Result<(CircleSizeMode, Option<f64>), PointInputError>> {
    let input = input.trim();
    if input.is_empty() || input.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
        let value = match current_mode {
            CircleSizeMode::Radius => preferences.radius,
            CircleSizeMode::Diameter => 2. * preferences.radius,
            _ => return None,
        };
        return Some(Ok((current_mode, Some(value))));
    }
    if let Some(mode) = CircleSizeMode::parse(input) {
        return Some(Ok((mode, None)));
    }
    let (mode, value, named) = if let Some((name, value)) = input
        .split_once('=')
        .or_else(|| input.split_once(char::is_whitespace))
        && let Some(mode) = CircleSizeMode::parse(name.trim())
    {
        (mode, value.trim(), true)
    } else {
        (current_mode, input, false)
    };
    let units = if mode == CircleSizeMode::Area {
        &LengthUnitSystem::Unset
    } else {
        units
    };
    let parsed = PointInput::parse_length_with_units(value, units)
        .or_else(|| named.then_some(Err(PointInputError::InvalidNumber)))?;
    Some(parsed.map(|value| (mode, Some(value))))
}

impl VibocerosApp {
    pub(super) fn circle_takes_length_measurement(&self) -> bool {
        match self.active_command {
            Some(InteractiveCommand::Circle { center: Some(_) })
            | Some(InteractiveCommand::CircleThreePointRadius { radius: None, .. }) => true,
            Some(InteractiveCommand::CircleSize { mode, .. })
            | Some(InteractiveCommand::CircleVertical {
                center: Some(_),
                radius: None,
                mode,
                ..
            })
            | Some(InteractiveCommand::CircleOrientation {
                normal_point: Some(_),
                mode,
                ..
            }) => mode != CircleSizeMode::Area,
            _ => false,
        }
    }

    pub(super) fn try_continue_circle(&mut self, input: &str) -> bool {
        if self.plane_prompt.is_some() || self.object_prompt.is_some() {
            return false;
        }
        if self.try_continue_circle_snap_projection(input) {
            return true;
        }
        let Some(state) = self.active_command else {
            return false;
        };
        if state == (InteractiveCommand::Circle { center: None }) {
            let option = input.trim().trim_start_matches('_');
            if option.eq_ignore_ascii_case("Deformable") {
                self.start_circle_deform();
                self.command_input.clear();
                return true;
            }
            if option.eq_ignore_ascii_case("Tangent") {
                self.start_tangent_circle(None);
                self.command_input.clear();
                return true;
            }
            if option.eq_ignore_ascii_case("AroundCurve") {
                self.start_circle_around();
                self.command_input.clear();
                return true;
            }
            if option.eq_ignore_ascii_case("FitPoints") {
                self.cancel_interactive_command(false);
                if !self.try_start_object_prompt("Circle FitPoints") {
                    self.execute_command("Circle FitPoints");
                }
                return true;
            }
            let next = if option.eq_ignore_ascii_case("2Point") {
                InteractiveCommand::CircleTwoPoint {
                    first: None,
                    vertical: false,
                }
            } else if option.eq_ignore_ascii_case("3Point") {
                InteractiveCommand::CircleThreePoint { points: [None; 2] }
            } else if option.eq_ignore_ascii_case("Vertical") {
                InteractiveCommand::CircleVertical {
                    center: None,
                    radius: None,
                    mode: self.commands.circle_preferences().mode(),
                }
            } else {
                return false;
            };
            self.command_input.clear();
            self.active_command = Some(next);
            self.push_log(next.prompt().to_owned());
            return true;
        }
        let option = input.trim().trim_start_matches(['_', '-']);
        if matches!(
            state,
            InteractiveCommand::CircleVertical { center: None, .. }
        ) && option.eq_ignore_ascii_case("Deformable")
        {
            return self.try_start_circle_deform("Circle Vertical Deformable");
        }
        if matches!(
            state,
            InteractiveCommand::CircleVertical { center: None, .. }
        ) && option.eq_ignore_ascii_case("2Point")
            || matches!(
                state,
                InteractiveCommand::CircleTwoPoint {
                    first: None,
                    vertical: false
                }
            ) && option.eq_ignore_ascii_case("Vertical")
        {
            self.set_circle_prompt(InteractiveCommand::CircleTwoPoint {
                first: None,
                vertical: true,
            });
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
            if let Some((name, value)) = option
                .split_once('=')
                .or_else(|| option.split_once(char::is_whitespace))
                && name.eq_ignore_ascii_case("Radius")
            {
                let radius = PointInput::parse_length_with_units(value, self.document.units())
                    .and_then(Result::ok);
                return self.accept_three_point_radius_value(first, second, radius);
            }
        }
        if let InteractiveCommand::CircleThreePointRadius {
            first,
            second,
            radius: None,
        } = state
        {
            let radius = if option.is_empty() || option.eq_ignore_ascii_case("Enter") {
                Some(self.commands.circle_preferences().radius)
            } else {
                let Some(value) = PointInput::parse_length_with_units(input, self.document.units())
                else {
                    return false;
                };
                value.ok()
            };
            return self.accept_three_point_radius_value(first, second, radius);
        }
        let (center, current_mode) = match state {
            InteractiveCommand::Circle {
                center: Some(center),
            } => (center, CircleSizeMode::Radius),
            InteractiveCommand::CircleSize { center, mode }
            | InteractiveCommand::CircleVertical {
                center: Some(center),
                mode,
                ..
            }
            | InteractiveCommand::CircleOrientation {
                center,
                normal_point: Some(_),
                mode,
            } => (center, mode),
            _ => return false,
        };
        if input
            .trim()
            .trim_start_matches(['_', '-'])
            .eq_ignore_ascii_case("Orientation")
        {
            let next = InteractiveCommand::CircleOrientation {
                center,
                normal_point: None,
                mode: current_mode,
            };
            self.set_circle_prompt(next);
            return true;
        }
        let Some(change) = parse_size_input(
            input,
            current_mode,
            self.document.units(),
            self.commands.circle_preferences(),
        ) else {
            return false;
        };
        let (mode, value) = match change {
            Ok(change) => change,
            Err(error) => {
                self.command_input.clear();
                self.push_log(format!("Error: {error}"));
                return true;
            }
        };
        self.command_input.clear();
        match state {
            InteractiveCommand::CircleVertical { radius, .. } => {
                if let Some(value) = value {
                    let measured =
                        matches!(mode, CircleSizeMode::Circumference | CircleSizeMode::Area);
                    if measured {
                        let argument = format!("{}={value}", mode.label());
                        self.finish_circle_vertical(state, &argument);
                    } else {
                        let radius = mode.radius(value);
                        if !radius.is_finite() || radius <= self.document.tolerance().absolute() {
                            self.push_log(
                                "Error: circle radius must exceed document tolerance".into(),
                            );
                            return true;
                        }
                        self.remember_circle_mode(mode);
                        self.set_circle_prompt(InteractiveCommand::CircleVertical {
                            center: Some(center),
                            radius: Some(radius),
                            mode,
                        });
                    }
                } else {
                    let radius = if mode == current_mode { radius } else { None };
                    self.remember_circle_mode(mode);
                    self.set_circle_prompt(InteractiveCommand::CircleVertical {
                        center: Some(center),
                        radius,
                        mode,
                    });
                }
            }
            InteractiveCommand::CircleOrientation { normal_point, .. } => {
                let next = InteractiveCommand::CircleOrientation {
                    center,
                    normal_point,
                    mode,
                };
                if let Some(value) = value {
                    self.execute_circle_orientation(next, &format!("{}={value}", mode.label()));
                } else {
                    self.remember_circle_mode(mode);
                    self.set_circle_prompt(next);
                }
            }
            _ => {
                let next = InteractiveCommand::CircleSize { center, mode };
                if let Some(value) = value {
                    self.finish_circle_size(next, &format!("{}={value}", mode.label()));
                } else {
                    self.remember_circle_mode(mode);
                    self.set_circle_prompt(next);
                }
            }
        }
        true
    }

    fn set_circle_prompt(&mut self, next: InteractiveCommand) {
        self.active_command = Some(next);
        self.command_input.clear();
        self.push_log(next.prompt().to_owned());
    }

    fn remember_circle_mode(&self, mode: CircleSizeMode) {
        if matches!(mode, CircleSizeMode::Radius | CircleSizeMode::Diameter) {
            self.commands
                .remember_circle_diameter(mode == CircleSizeMode::Diameter);
        }
    }

    pub(super) fn circle_preview(&self) -> Option<(PointConstruction, Frame3)> {
        if self.plane_prompt.is_some() {
            return None;
        }
        let construction = match self.active_command? {
            InteractiveCommand::Circle {
                center: Some(center),
            } => PointConstruction::Center {
                center,
                mode: CircleSizeMode::Radius,
            },
            InteractiveCommand::CircleSize { center, mode } => {
                PointConstruction::Center { center, mode }
            }
            InteractiveCommand::CircleVertical {
                center: Some(center),
                radius,
                mode,
            } => PointConstruction::Vertical {
                center,
                radius,
                mode,
            },
            InteractiveCommand::CircleTwoPoint {
                first: Some(first),
                vertical,
            } => PointConstruction::Diameter { first, vertical },
            InteractiveCommand::CircleThreePoint {
                points: [Some(first), Some(second)],
            } => PointConstruction::ThreePoint { first, second },
            InteractiveCommand::CircleThreePointRadius {
                first,
                second,
                radius,
            } => PointConstruction::ThreePointRadius {
                first,
                second,
                radius,
            },
            InteractiveCommand::CircleOrientation {
                center,
                normal_point: Some(normal_point),
                mode,
            } => PointConstruction::Oriented {
                center,
                normal_point,
                mode,
            },
            _ => return None,
        };
        let plane = self
            .drafting_plane
            .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane());
        Some((construction, plane))
    }

    pub(super) fn circle_snap_plane(&self) -> Option<Frame3> {
        if self.plane_prompt.is_some() {
            return None;
        }
        if let Some(session) = &self.circle_deform_session {
            return session.input.getter().snap_projection_plane();
        }
        if let Some(session) = &self.circle_around_session {
            return session.snap_projection_plane();
        }
        if let Some(session) = &self.solid_axial_session {
            return session.input.snap_projection_plane();
        }
        if let Some(session) = &self.torus_session {
            return session.input.snap_projection_plane();
        }
        let (construction, plane) = self.circle_preview()?;
        match construction {
            PointConstruction::Center {
                center,
                mode: CircleSizeMode::Radius | CircleSizeMode::Diameter,
            }
            | PointConstruction::Vertical {
                center,
                mode: CircleSizeMode::Radius | CircleSizeMode::Diameter,
                ..
            } => Some(plane.with_origin(center)),
            PointConstruction::Oriented {
                center,
                normal_point,
                mode: CircleSizeMode::Radius | CircleSizeMode::Diameter,
            } => Frame3::try_from_normal(
                center,
                center.vector_to(normal_point).ok()?,
                self.document.tolerance(),
            )
            .ok(),
            _ => None,
        }
    }

    pub(super) fn try_continue_circle_snap_projection(&mut self, input: &str) -> bool {
        let input = input.trim().trim_start_matches(['_', '-']);
        let (name, value) = input
            .split_once('=')
            .or_else(|| input.split_once(char::is_whitespace))
            .map_or((input, None), |(name, value)| {
                (name.trim(), Some(value.trim()))
            });
        if !name.eq_ignore_ascii_case("ProjectOsnap") || self.circle_snap_plane().is_none() {
            return false;
        }
        self.command_input.clear();
        self.circle_project_osnap = match value {
            None => !self.circle_project_osnap,
            Some(word) if word.trim_start_matches('_').eq_ignore_ascii_case("Yes") => true,
            Some(word) if word.trim_start_matches('_').eq_ignore_ascii_case("No") => false,
            Some(_) => {
                self.push_log("ProjectOsnap expects Yes or No".into());
                return true;
            }
        };
        let value = if self.circle_project_osnap {
            "Yes"
        } else {
            "No"
        };
        self.push_log(format!("ProjectOsnap={value}"));
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
        self.finish_circle_command(state, &command)
    }

    pub(super) fn finish_circle_command(
        &mut self,
        state: InteractiveCommand,
        command: &str,
    ) -> bool {
        let drafting_plane = self.drafting_plane;
        self.active_command = None;
        self.command_input.clear();
        let success = self.try_execute_command(command);
        if !success {
            self.drafting_plane = drafting_plane;
            self.set_circle_prompt(state);
        }
        success
    }

    fn accept_three_point_radius_value(
        &mut self,
        first: Point3,
        second: Point3,
        radius: Option<f64>,
    ) -> bool {
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
        argument: &str,
    ) -> bool {
        let InteractiveCommand::CircleVertical {
            center: Some(center),
            ..
        } = state
        else {
            unreachable!("vertical circle requires a center");
        };
        self.finish_circle_command(
            state,
            &format!("Circle Vertical {} {argument}", format_model_point(center)),
        )
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
        self.finish_circle_command(state, &command)
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
        self.finish_circle_command(state, &command)
    }
}
