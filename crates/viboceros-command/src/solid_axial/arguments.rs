//! Complete scripts using the same staged base and height getters.
use super::*;
use crate::{
    circle_around_input, circle_tangent_input,
    curve_reference::{CapturedCurve, parse_edge, perpendicular_frame},
    parse_finite_real, parse_point,
};
use viboceros_document::Document;
use viboceros_drafting::PointInput;

pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    kind: Kind,
    plane: Frame3,
    p: Preferences,
) -> Result<Input, CommandError> {
    let usage = kind.usage();
    let mut input = Input::new(kind, plane, p);
    let mut rest = args;
    let mut guide = None;
    while let Some((&word, tail)) = rest.split_first() {
        if input.definition.is_some() {
            return Err(CommandError::Usage(usage));
        }
        if input.needs_guide() {
            if input.option(word)? {
                rest = tail;
                continue;
            }
            let (key, value) = word
                .trim_start_matches('_')
                .split_once('=')
                .ok_or(CommandError::Usage(usage))?;
            let (id, edge) = if option_name_eq(key, "Curve") {
                (value.parse().map_err(|_| CommandError::Usage(usage))?, None)
            } else if option_name_eq(key, "Edge") {
                let (id, index) = parse_edge(value).ok_or(CommandError::Usage(usage))?;
                (id, Some(index))
            } else {
                return Err(CommandError::Usage(usage));
            };
            guide = Some(CapturedCurve::capture(doc, id, edge)?);
            input.select_guide();
            rest = tail;
            continue;
        }
        if input.at_start() && option_name_eq(word, "Tangent") {
            let mut tangent = circle_tangent_input::Input::new(
                plane,
                doc.tolerance(),
                circle_around_input::Preferences {
                    radius: input.preferences.radius,
                    ..Default::default()
                },
            );
            rest = tail;
            loop {
                let (count, circle) = tangent.argument(doc, rest)?;
                rest = &rest[count..];
                if let Some(circle) = circle {
                    input.reference_circle(circle, true)?;
                    break;
                }
            }
            continue;
        }
        if input.at_start() && option_name_eq(word, "FitPoints") {
            input.fit_points(doc)?;
            rest = tail;
            continue;
        }
        if input.option(word)? {
            rest = tail;
            continue;
        }
        if option_name_eq(word, "Enter") {
            input.enter()?;
            rest = tail;
            continue;
        }
        if word == "0" && input.zero_is_point() {
            input.point(plane.origin())?;
            rest = tail;
            continue;
        }
        if let Some(value) = input
            .wants_number()
            .then(|| parse_finite_real(word).ok())
            .flatten()
        {
            input.number(value)?;
            rest = tail;
        } else {
            let (point, count) = if word == "0" && !input.wants_number() {
                (plane.origin(), 1)
            } else if kind == Kind::Tube
                && let Some(point) = PointInput::parse_with_units(word, doc.units())
            {
                let point = point
                    .and_then(|point| point.resolve(plane, input.anchor()))
                    .map_err(|_| CommandError::InvalidNumber(word.into()))?;
                (point, 1)
            } else {
                parse_point(rest)?
            };
            if input.at_start() && input.preferences.direction == Direction::AroundCurve {
                let source = guide.as_ref().ok_or(CommandError::Usage(usage))?;
                input.around_frame(perpendicular_frame(source.curve_ref(), point)?);
            } else {
                input.point(point)?;
            }
            rest = &rest[count..];
        }
    }
    if input.definition.is_none() {
        return Err(CommandError::Usage(usage));
    }
    Ok(input)
}
