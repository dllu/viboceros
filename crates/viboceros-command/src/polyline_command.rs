//! Script entry point for straight and staged polylines.
use super::*;
use crate::polyline_input::{Input, OptionResult, Preferences, segments::line_polyline};
use std::sync::Arc;
use viboceros_drafting::PointInput;

pub(super) struct PolylineCommand(pub Arc<remembered::Remembered<Preferences>>);

impl Command for PolylineCommand {
    fn name(&self) -> &'static str {
        "Polyline"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["PLine"]
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
        if let Ok(vertices) = legacy_points(arguments) {
            let geometry = Geometry::Polyline(line_polyline(vertices, document.tolerance())?);
            return commit(document, geometry);
        }

        let plane = context.construction_plane;
        let tolerance = document.tolerance();
        let mut draft = Input::new(plane, self.0.get());
        let mut remaining = arguments;
        let mut result = None;
        while let Some((&word, tail)) = remaining.split_first() {
            if let Some(option) = draft.option(word, document.units(), tolerance)? {
                if let OptionResult::Finish { close } = option {
                    result = Some(draft.finish(close, tolerance)?);
                    remaining = tail;
                    break;
                }
                remaining = tail;
                continue;
            }
            if word != "0" || !draft.origin_shortcut() {
                if let Some(value) = draft.parse_value(word, document.units()) {
                    result = draft.value(value?, tolerance)?;
                    remaining = tail;
                    if result.is_some() {
                        break;
                    }
                    continue;
                }
            }
            let (point, consumed) = if word == "0" && draft.origin_shortcut() {
                (plane.origin(), 1)
            } else if let Some(parsed) = PointInput::parse_with_units(word, document.units()) {
                let point = parsed
                    .and_then(|value| value.resolve(plane, draft.anchor()))
                    .map_err(|_| CommandError::InvalidNumber(word.to_owned()))?;
                (point, 1)
            } else {
                parse_point(remaining)?
            };
            result = draft.pick(point, tolerance)?;
            remaining = &remaining[consumed..];
            if result.is_some() {
                break;
            }
        }
        // Enter following an explicit return to the start is harmless.
        if let [word] = remaining
            && option_name_eq(word, "Enter")
        {
            remaining = &[];
        }
        require_consumed(remaining, 0, polyline_input::USAGE)?;
        let geometry = match result {
            Some(geometry) => geometry,
            None => draft.finish(false, tolerance)?,
        };
        let result = commit(document, geometry)?;
        self.0.set(draft.preferences());
        Ok(result)
    }
}

fn legacy_points(mut arguments: &[&str]) -> Result<Vec<Point3>, CommandError> {
    let mut vertices = Vec::new();
    while !arguments.is_empty() {
        let (point, consumed) = parse_point(arguments)?;
        vertices.push(point);
        arguments = &arguments[consumed..];
    }
    Ok(vertices)
}

fn commit(document: &mut Document, geometry: Geometry) -> Result<String, CommandError> {
    let summary = match &geometry {
        Geometry::Polyline(polyline) => format!(
            "{} vertices, {} segments",
            polyline.vertices().len(),
            polyline.segment_count(),
        ),
        Geometry::PolyCurve(curve) => format!("{} segments", curve.segments().len()),
        _ => unreachable!("polyline draft output"),
    };
    let closed = geometry
        .curve_ref()
        .is_some_and(|curve| curve.is_closed().unwrap_or(false));
    let id = document.add_geometry(geometry)?;
    Ok(format!(
        "Added {}polyline {id} ({summary})",
        if closed { "closed " } else { "" }
    ))
}

impl CommandRegistry {
    pub fn polyline_preferences(&self) -> Preferences {
        self.polyline_preferences.get()
    }

    pub fn remember_polyline_preferences(&self, value: Preferences) {
        self.polyline_preferences.set(value);
    }
}
