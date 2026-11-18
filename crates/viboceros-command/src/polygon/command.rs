//! Registry entry point for staged Polygon input and the positional shorthand.
use super::*;
use crate::{
    Command, CommandContext, CommandRegistry,
    curve_reference::{CapturedCurve, parse_edge, perpendicular_frame},
    parse_point,
    remembered::Remembered,
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry};
use viboceros_drafting::PointInput;

pub(crate) struct PolygonCommand(pub Arc<Remembered<Preferences>>);

impl Command for PolygonCommand {
    fn name(&self) -> &'static str {
        "Polygon"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Poly"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        mut arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let mut input = Input::new(plane, self.0.get(), document.tolerance());
        let legacy = arguments.first().is_some_and(|word| {
            *word != "0" && word.trim_start_matches('_').parse::<usize>().is_ok()
        });
        if legacy {
            input.set_sides(arguments[0])?;
            input.preferences.mode = Mode::Inscribed;
            arguments = &arguments[1..];
        }
        let mut guide: Option<CapturedCurve> = None;

        while let Some((&word, tail)) = arguments.split_first() {
            if input.option(word)? {
                arguments = tail;
                continue;
            }
            if input.needs_guide() && guide.is_none() {
                let (key, value) = word.split_once('=').ok_or(CommandError::Usage(USAGE))?;
                let (id, edge) = if option_name_eq(key, "Edge") {
                    let (id, edge) = parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                    (id, Some(edge))
                } else if option_name_eq(key, "Curve") {
                    (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
                guide = Some(CapturedCurve::capture(document, id, edge)?);
                arguments = tail;
                continue;
            }
            let separated_point = arguments.len() >= 3
                && arguments[..3]
                    .iter()
                    .all(|word| word.parse::<f64>().is_ok());
            let scalar = PointInput::parse_length_with_units(word, document.units());
            let (definition, consumed) = if option_name_eq(word, "Enter") {
                (input.enter(None)?, 1)
            } else if input.takes_size()
                && !separated_point
                && !(word == "0" && input.accepts_origin())
                && let Some(value) = scalar
            {
                let value = value.map_err(|_| CommandError::InvalidNumber(word.to_owned()))?;
                if legacy && tail.is_empty() {
                    input.checked_size(value)?;
                    input.number(value)?;
                    (input.enter(None)?, 1)
                } else {
                    (input.number(value)?, 1)
                }
            } else {
                let (point, count) = if word == "0" && input.accepts_origin() && !separated_point {
                    (plane.origin(), 1)
                } else if !legacy
                    && !separated_point
                    && let Some(value) = PointInput::parse_with_units(word, document.units())
                {
                    let point = value
                        .and_then(|p| p.resolve(plane, input.anchor()))
                        .map_err(|_| CommandError::InvalidNumber(word.to_owned()))?;
                    (point, 1)
                } else {
                    parse_point(arguments)?
                };
                if let Some(guide) = guide.take() {
                    input.set_guide(perpendicular_frame(guide.curve_ref(), point)?);
                    (None, count)
                } else {
                    (input.point(point)?, count)
                }
            };
            arguments = &arguments[consumed..];
            if let Some(definition) = definition {
                if !arguments.is_empty() {
                    return Err(CommandError::Usage(USAGE));
                }
                let polyline = definition.polyline(document.tolerance())?;
                let perimeter = polyline.length()?;
                let id = document.add_geometry(Geometry::Polyline(polyline))?;
                self.0.set(input.preferences());
                return Ok(format!(
                    "Added {}-sided polygon {id} (perimeter {perimeter:.6})",
                    definition.sides()
                ));
            }
        }
        Err(CommandError::Usage(USAGE))
    }
}

impl CommandRegistry {
    pub fn polygon_preferences(&self) -> Preferences {
        self.polygon_preferences.get()
    }

    pub fn remember_polygon_preferences(&self, preferences: Preferences) {
        self.polygon_preferences.set(preferences);
    }
}
