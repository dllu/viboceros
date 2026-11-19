//! Ribbon options shared by the curve and side getters.
use super::*;
use crate::app::object_selection::ObjectPromptPhase;
use viboceros_command::{CommandContext, CommandError};
use viboceros_drafting::{PointConstraintInput, PointFilter, PointInput};
use viboceros_geometry::LengthUnitSystem;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum Getter {
    Distance,
    DistanceEnd(Point3),
    Corner,
    OutputLayer,
    Tolerance,
}

impl Getter {
    pub(in crate::app) fn name(self) -> &'static str {
        match self {
            Self::Distance | Self::DistanceEnd(_) => "Distance",
            Self::Corner => "Corner",
            Self::OutputLayer => "OutputLayer",
            Self::Tolerance => "Tolerance",
        }
    }

    pub(in crate::app) fn hint(self) -> &'static str {
        match self {
            Self::Distance => "Enter a positive distance or pick the first point",
            Self::DistanceEnd(_) => "Pick the second distance point",
            Self::Corner => "Choose Sharp, Round, Smooth, Chamfer or None",
            Self::OutputLayer => "Choose Current or Input",
            Self::Tolerance => "Enter a tolerance or choose Default",
        }
    }

    pub(in crate::app) fn takes_point(self) -> bool {
        matches!(self, Self::Distance | Self::DistanceEnd(_))
    }

    fn anchor(self) -> Option<Point3> {
        match self {
            Self::DistanceEnd(start) => Some(start),
            _ => None,
        }
    }
}

impl VibocerosApp {
    pub(super) fn ribbon_source_getter(&self) -> Option<Getter> {
        match self.object_prompt.as_ref()?.phase {
            ObjectPromptPhase::RibbonValue(getter) => Some(getter),
            _ => None,
        }
    }

    fn ribbon_source_input(&self, context: CommandContext) -> Result<solver::Input, CommandError> {
        let command = self
            .object_prompt
            .as_ref()
            .unwrap()
            .command_override
            .as_deref();
        solver::parse(
            &command
                .unwrap_or("Ribbon")
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>(),
            self.commands.ribbon_options(),
            self.document.units(),
            context,
        )
    }

    pub(in crate::app) fn continue_ribbon_source_options(&mut self, text: &str) -> Option<bool> {
        let pending = self.object_prompt.as_ref()?;
        if pending.description.command != "Ribbon" {
            return None;
        }
        let getter = self.ribbon_source_getter();
        if getter.is_none() && !is_option(text) {
            return None;
        }
        if is_cancel(text) {
            self.cancel_interactive_command(true);
            return Some(true);
        }
        if getter.is_some_and(|g| point_input(g, text, self.document.units())) {
            return Some(false);
        }
        let context = CommandContext {
            construction_plane: self
                .drafting_plane
                .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane()),
        };
        let result = self
            .ribbon_source_input(context)
            .and_then(|current| edit(text, current, getter, self.document.units(), context));
        match result {
            Ok((input, getter)) => self.update_ribbon_source(input, getter, context),
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        Some(true)
    }

    fn update_ribbon_source(
        &mut self,
        input: solver::Input,
        getter: Option<Getter>,
        context: CommandContext,
    ) {
        let pending = self.object_prompt.as_mut().unwrap();
        pending.command_override = Some(ribbon_command_line(input));
        pending.phase = getter.map_or(ObjectPromptPhase::Selecting, ObjectPromptPhase::RibbonValue);
        let point_getter = getter.filter(|g| g.takes_point());
        self.active_command =
            point_getter.map(|g| InteractiveCommand::Ribbon { anchor: g.anchor() });
        self.drafting_plane = point_getter.map(|_| context.construction_plane);
        self.commands.remember_ribbon_options(input.options);
        self.point_constraint = None;
        self.point_filter = None;
        self.command_input.clear();
        self.log_object_prompt();
    }

    pub(in crate::app) fn continue_ribbon(&mut self, text: &str) -> bool {
        let Some(session) = self.ribbon_session.as_ref() else {
            return false;
        };
        if self.plane_prompt.is_some() || viboceros_command::interface::parse(text).is_some() {
            return false;
        }
        if is_cancel(text) {
            self.cancel_interactive_command(true);
            return true;
        }
        if is_subcurve_option(text) {
            self.push_log("SubCrv is available while selecting the Ribbon source".into());
            self.command_input.clear();
            return true;
        }
        let getter = session.menu;
        let units = self.document.units();
        let recognized_command = text
            .split_whitespace()
            .next()
            .is_some_and(|name| self.commands.recognizes(name));
        if getter.is_some_and(|g| point_input(g, text, units))
            || (getter.is_none()
                && !is_option(text)
                && (recognized_command
                    || (solver::length(text, units).is_err()
                        && PointInput::parse_with_units(text, units).is_some())))
        {
            return false;
        }
        if !self.validate_ribbon() {
            return true;
        }
        if is_enter(text) && getter.is_none() {
            self.cancel_interactive_command(true);
            return true;
        }
        let session = self.ribbon_session.as_ref().unwrap();
        let context = CommandContext {
            construction_plane: session.plane,
        };
        let current = solver::Input {
            options: session.options,
            side: None,
            through_point: session.through_point,
        };
        match edit(text, current, getter, self.document.units(), context) {
            Ok((input, getter)) => {
                self.update_ribbon_side(input, getter);
                if let Some(point) = input.side {
                    self.accept_ribbon_point(point);
                }
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        self.command_input.clear();
        true
    }

    fn update_ribbon_side(&mut self, input: solver::Input, getter: Option<Getter>) {
        let session = self.ribbon_session.as_mut().unwrap();
        session.options = input.options;
        session.through_point = input.through_point;
        session.menu = getter;
        self.active_command = Some(InteractiveCommand::Ribbon {
            anchor: getter.and_then(Getter::anchor),
        });
        self.commands.remember_ribbon_options(input.options);
        self.point_constraint = None;
        self.point_filter = None;
        self.command_input.clear();
        self.ribbon_prompt();
    }

    pub(super) fn accept_ribbon_distance(&mut self, getter: Getter, point: Point3) -> bool {
        if !getter.takes_point() {
            return false;
        }
        let (mut input, context) = if let Some(session) = &self.ribbon_session {
            (
                solver::Input {
                    options: session.options,
                    side: None,
                    through_point: session.through_point,
                },
                CommandContext {
                    construction_plane: session.plane,
                },
            )
        } else {
            let context = CommandContext {
                construction_plane: self
                    .drafting_plane
                    .unwrap_or_else(|| self.viewports[self.active_viewport].construction_plane()),
            };
            match self.ribbon_source_input(context) {
                Ok(input) => (input, context),
                Err(error) => {
                    self.push_log(format!("Error: {error}"));
                    return false;
                }
            }
        };
        let next = if let Some(start) = getter.anchor() {
            match start.distance_to(point) {
                Ok(distance) if distance > 0. => {
                    input.options.distance = distance;
                    input.through_point = false;
                    None
                }
                _ => {
                    self.push_log("Ribbon distance must be positive and finite".into());
                    return false;
                }
            }
        } else {
            Some(Getter::DistanceEnd(point))
        };
        if self.ribbon_session.is_some() {
            self.update_ribbon_side(input, next);
        } else {
            self.update_ribbon_source(input, next, context);
        }
        true
    }
}

fn option_name(text: &str) -> &str {
    text.split_whitespace()
        .next()
        .unwrap_or("")
        .split('=')
        .next()
        .unwrap()
        .trim_start_matches(['_', '-'])
}

fn is_option(text: &str) -> bool {
    [
        "Distance",
        "Corner",
        "OutputLayer",
        "Tolerance",
        "Loose",
        "InCPlane",
        "BothSides",
        "ThroughPoint",
        "Trim",
    ]
    .iter()
    .any(|name| option_name(text).eq_ignore_ascii_case(name))
}

fn is_enter(text: &str) -> bool {
    text.is_empty() || text.trim_start_matches('_').eq_ignore_ascii_case("Enter")
}

fn is_cancel(text: &str) -> bool {
    text.trim_start_matches(['_', '-'])
        .eq_ignore_ascii_case("Cancel")
}

fn point_input(getter: Getter, text: &str, units: &LengthUnitSystem) -> bool {
    getter.takes_point()
        && (matches!(PointInput::parse_with_units(text, units), Some(Ok(_)))
            || (getter.anchor().is_some()
                && PointConstraintInput::parse_with_units(text, units).is_some())
            || PointFilter::parse(text).is_some())
}

fn edit(
    text: &str,
    mut current: solver::Input,
    getter: Option<Getter>,
    units: &LengthUnitSystem,
    context: CommandContext,
) -> Result<(solver::Input, Option<Getter>), CommandError> {
    if is_enter(text) {
        return Ok((current, None));
    }
    let name = option_name(text);
    let bare = !text.contains('=') && text.split_whitespace().count() == 1;
    if getter.is_none()
        && current.options.loose
        && ["Corner", "Tolerance", "Trim"]
            .iter()
            .any(|opt| name.eq_ignore_ascii_case(opt))
    {
        return Err(CommandError::Usage(
            "Set Loose=No to edit Corner, Tolerance, or Trim",
        ));
    }
    let command = if let Some(getter) = getter {
        format!("{}={text}", getter.name())
    } else if bare {
        let bare_option = [
            ("Distance", Getter::Distance),
            ("Corner", Getter::Corner),
            ("OutputLayer", Getter::OutputLayer),
            ("Tolerance", Getter::Tolerance),
        ];
        if let Some((_, getter)) = bare_option
            .into_iter()
            .find(|(opt, _)| name.eq_ignore_ascii_case(opt))
        {
            return Ok((current, Some(getter)));
        }
        if name.eq_ignore_ascii_case("Loose") {
            current.options.loose = !current.options.loose;
            return Ok((current, None));
        }
        if name.eq_ignore_ascii_case("InCPlane") {
            current.options.in_cplane = !current.options.in_cplane;
            return Ok((current, None));
        }
        if name.eq_ignore_ascii_case("Trim") {
            current.options.trim = !current.options.trim;
            return Ok((current, None));
        }
        text.to_owned()
    } else {
        text.to_owned()
    };
    let words = command.split_whitespace().collect::<Vec<_>>();
    Ok((
        solver::parse_with_defaults(&words, current, units, context)?,
        None,
    ))
}
