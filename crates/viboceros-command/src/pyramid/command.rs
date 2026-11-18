//! Registry entry point for staged input and the existing explicit-axis form.
use super::*;
use crate::{
    Command, CommandContext, CommandError, PYRAMID_USAGE, axial_primitive_frame,
    curve_reference::{CapturedCurve, parse_edge, perpendicular_frame},
    option_name_eq, parse_point, parse_pyramid_options,
    remembered::Remembered,
};
use viboceros_document::{Document, Geometry};
use viboceros_drafting::{PointConstraintInput, PointConstraintState, PointInput};

#[derive(Default)]
pub(crate) struct PyramidCommand {
    preferences: Remembered<Preferences>,
}

impl Command for PyramidCommand {
    fn name(&self) -> &'static str {
        "Pyramid"
    }

    fn pyramid_options(&self) -> Option<Preferences> {
        Some(self.preferences.get())
    }

    fn remember_pyramid_options(&self, value: Preferences) {
        self.preferences.set(value);
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if arguments
            .first()
            .is_some_and(|word| word.parse::<usize>().is_ok())
        {
            return run_positional(doc, arguments);
        }
        let (definition, preferences) = self.staged(doc, arguments, context)?;
        let brep = definition.brep(doc.tolerance())?;
        let id = doc.add_geometry(Geometry::Brep(brep))?;
        self.preferences.set(preferences);
        Ok(format!("Created Pyramid {id}"))
    }
}

impl PyramidCommand {
    fn staged(
        &self,
        doc: &Document,
        mut arguments: &[&str],
        context: CommandContext,
    ) -> Result<(Definition, Preferences), CommandError> {
        let plane = context.construction_plane;
        let mut input = Input::new(plane, self.preferences.get(), doc.tolerance());
        let mut guide: Option<CapturedCurve> = None;
        let mut constraint = None;
        while let Some((&word, tail)) = arguments.split_first() {
            if let Some(source) = word.split_once('=').filter(|(name, _)| {
                input.needs_guide()
                    && (option_name_eq(name, "Curve") || option_name_eq(name, "Edge"))
            }) {
                let (id, edge) = if option_name_eq(source.0, "Edge") {
                    let (id, index) =
                        parse_edge(source.1).ok_or(CommandError::Usage(PYRAMID_USAGE))?;
                    (id, Some(index))
                } else {
                    (
                        source
                            .1
                            .parse()
                            .map_err(|_| CommandError::Usage(PYRAMID_USAGE))?,
                        None,
                    )
                };
                guide = Some(CapturedCurve::capture(doc, id, edge)?);
                arguments = tail;
                continue;
            }
            if input.option(word)? {
                arguments = tail;
                continue;
            }
            let invalid_number = |_| CommandError::InvalidNumber(word.to_owned());
            let scalar = PointInput::parse_length_with_units(word, doc.units());
            let (definition, consumed) = if option_name_eq(word, "Enter") {
                (input.enter()?, 1)
            } else if input.wants_number()
                && let Some(value) = scalar
            {
                (input.number(value.map_err(invalid_number)?)?, 1)
            } else {
                let (point, consumed) = match parse_point(arguments) {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        if let Some(anchor) = input.anchor()
                            && let Some(value) =
                                PointConstraintInput::parse_with_units(word, doc.units())
                        {
                            let state = constraint.get_or_insert_with(|| {
                                let state = PointConstraintState::new(anchor, plane);
                                input
                                    .mouse_plane()
                                    .map_or(state, |p| state.with_distance_plane(p.z_axis()))
                            });
                            state
                                .set(value.map_err(invalid_number)?, plane)
                                .map_err(|_| CommandError::InvalidNumber(word.into()))?;
                            arguments = tail;
                            continue;
                        }
                        let point = PointInput::parse_with_units(word, doc.units())
                            .ok_or(error)?
                            .and_then(|point| point.resolve(plane, input.anchor()))
                            .map_err(invalid_number)?;
                        (point, 1)
                    }
                };
                let point = constraint
                    .take()
                    .map_or(Ok(point), |state| state.apply_typed(point))
                    .map_err(|_| CommandError::InvalidNumber(word.into()))?;
                if input.needs_guide() {
                    let guide = guide.as_ref().ok_or(CommandError::Usage(PYRAMID_USAGE))?;
                    input.set_guide(perpendicular_frame(guide.curve_ref(), point)?)?;
                    (None, consumed)
                } else {
                    (input.point(point)?, consumed)
                }
            };
            arguments = &arguments[consumed..];
            if let Some(definition) = definition {
                return if arguments.is_empty() {
                    Ok((definition, input.preferences()))
                } else {
                    Err(CommandError::Usage(PYRAMID_USAGE))
                };
            }
        }
        Err(CommandError::Usage(PYRAMID_USAGE))
    }
}

fn run_positional(doc: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
    let options = parse_pyramid_options(arguments)?;
    let tolerance = doc.tolerance();
    let (frame, radius) =
        axial_primitive_frame(options.center, options.base_radius, options.axis, tolerance)?;
    let pyramid = Brep::try_pyramid(
        frame,
        options.side_count,
        radius,
        options.height,
        options.solid,
        tolerance,
    )?;
    let id = doc.add_geometry(Geometry::Brep(pyramid))?;
    Ok(format!(
        "Added {}-sided {} B-rep pyramid {id} (radius {radius:.6}, height {:.6})",
        options.side_count,
        if options.solid { "closed" } else { "open" },
        options.height
    ))
}
