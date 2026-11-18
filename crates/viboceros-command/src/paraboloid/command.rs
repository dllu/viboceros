//! Complete command invocations and the original positional script form.
use super::*;
use crate::{Command, CommandContext, CommandError, parse_point, remembered::Remembered};
use viboceros_document::{Document, Geometry};
use viboceros_drafting::{PointConstraintInput, PointInput};

const USAGE: &str = "Paraboloid [Focus|Vertex] first-point [Direction] second-point end-point [MarkFocus=Yes|No] [Solid=Yes|No]";

#[derive(Default)]
pub(crate) struct ParaboloidCommand {
    preferences: Remembered<Preferences>,
}

impl Command for ParaboloidCommand {
    fn name(&self) -> &'static str {
        "Paraboloid"
    }

    fn paraboloid_options(&self) -> Option<Preferences> {
        Some(self.preferences.get())
    }

    fn remember_paraboloid_options(&self, value: Preferences) {
        self.preferences.set(value);
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let legacy = positional_input(arguments, document.tolerance());
        let (definition, marker_first) = match legacy {
            Some(definition) => (definition?, true),
            None => (self.staged_input(document, arguments, context)?, false),
        };
        let brep = definition.brep()?;
        let marker = definition.focus_marker().map(Geometry::Point);
        let (id, focus_id) = if marker_first {
            let focus_id = marker.map(|g| document.add_geometry(g)).transpose()?;
            (document.add_geometry(Geometry::Brep(brep))?, focus_id)
        } else {
            let id = document.add_geometry(Geometry::Brep(brep))?;
            (id, marker.map(|g| document.add_geometry(g)).transpose()?)
        };
        if !marker_first {
            self.preferences.set(definition.preferences);
        }
        let focus_suffix = focus_id
            .map(|focus_id| format!(" and focus point {focus_id}"))
            .unwrap_or_default();
        let Dimensions {
            radius,
            height,
            focal_distance,
            ..
        } = definition.dimensions;
        let closure = if definition.preferences.solid {
            "closed"
        } else {
            "open"
        };
        Ok(format!(
            "Added {closure} B-rep paraboloid {id}{focus_suffix} (radius {radius:.6}, height {height:.6}, focus distance {focal_distance:.6})",
        ))
    }
}

impl ParaboloidCommand {
    fn staged_input(
        &self,
        document: &Document,
        mut arguments: &[&str],
        context: CommandContext,
    ) -> Result<Definition, CommandError> {
        let mut input = Input::new(self.preferences.get(), document.tolerance());
        let plane = context.construction_plane;
        let mut constraint = None;
        while let Some(&word) = arguments.first() {
            if input.option(word)? {
                arguments = &arguments[1..];
                continue;
            }
            let invalid_number = || CommandError::InvalidNumber(word.to_owned());
            let (point, consumed) = match parse_point(arguments) {
                Ok(parsed) => parsed,
                Err(error) => {
                    if let Some(state) = input.point_constraint(plane)
                        && let Some(value) =
                            PointConstraintInput::parse_with_units(word, document.units())
                    {
                        constraint
                            .get_or_insert(state)
                            .set(value.map_err(|_| invalid_number())?, plane)
                            .map_err(|_| invalid_number())?;
                        arguments = &arguments[1..];
                        continue;
                    }
                    let point = PointInput::parse_with_units(word, document.units())
                        .ok_or(error)?
                        .and_then(|point| point.resolve(plane, input.anchor()))
                        .map_err(|_| invalid_number())?;
                    (point, 1)
                }
            };
            let point = constraint
                .take()
                .map_or(Ok(point), |state| state.apply_typed(point))
                .map_err(|_| invalid_number())?;
            arguments = &arguments[consumed..];
            if let Some(definition) = input.point(point)? {
                return if arguments.is_empty() {
                    Ok(definition)
                } else {
                    Err(CommandError::Usage(USAGE))
                };
            }
        }
        Err(CommandError::Usage(USAGE))
    }
}

fn positional_input(
    mut arguments: &[&str],
    tolerance: Tolerance,
) -> Option<Result<Definition, CommandError>> {
    let mut input = Input::new(Preferences::default(), tolerance);
    if arguments.first().is_some_and(|word| {
        let word = word.trim_start_matches('_');
        word.eq_ignore_ascii_case("Focus") || word.eq_ignore_ascii_case("Vertex")
    }) {
        input.option(arguments[0]).ok()?;
        arguments = &arguments[1..];
    }
    let (first, consumed) = parse_point(arguments).ok()?;
    let (second, consumed_second) = parse_point(&arguments[consumed..]).ok()?;
    let (end, consumed_end) = parse_point(&arguments[consumed + consumed_second..]).ok()?;
    let mut seen = Preferences::default();
    for word in &arguments[consumed + consumed_second + consumed_end..] {
        let Some((name, _)) = word.split_once('=') else {
            return Some(Err(CommandError::Usage(USAGE)));
        };
        let seen = if crate::option_name_eq(name, "MarkFocus") {
            &mut seen.mark_focus
        } else if crate::option_name_eq(name, "Solid") {
            &mut seen.solid
        } else {
            return Some(Err(CommandError::Usage(USAGE)));
        };
        if *seen {
            return Some(Err(CommandError::Usage(USAGE)));
        }
        *seen = true;
        if input.option(word).is_err() {
            return Some(Err(CommandError::Usage(USAGE)));
        }
    }
    Some((|| {
        input.point(first)?;
        input.point(second)?;
        Ok(input.point(end)?.expect("three positional points"))
    })())
}
