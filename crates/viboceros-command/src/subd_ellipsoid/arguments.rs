//! Complete scripts retain guide identity and reject trailing or partial input.
use super::*;
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    p: Preferences,
) -> Result<(Input, Vec<viboceros_document::ObjectId>), CommandError> {
    let mut input = Input::new(plane, p, doc.tolerance());
    let mut i = 0;
    let mut release = vec![];
    while i < args.len() {
        if input.complete() {
            return Err(CommandError::Usage(USAGE));
        }
        let word = args[i];
        if input.option(word)? {
            i += 1;
            continue;
        }
        if input.getter.requests_curve() {
            let (key, value) = word
                .trim_start_matches(['_', '-'])
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            let (id, edge) =
                if key.eq_ignore_ascii_case("Curve") || key.eq_ignore_ascii_case("PickedCurve") {
                    (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
                } else if key.eq_ignore_ascii_case("Edge") {
                    let (id, e) = crate::curve_reference::parse_edge(value)
                        .ok_or(CommandError::Usage(USAGE))?;
                    (id, Some(e))
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
            let source = crate::curve_reference::CapturedCurve::capture(doc, id, edge)?;
            i += 1;
            let (center, n) = crate::parse_point(&args[i..])?;
            i += n;
            input
                .getter
                .around_curve(crate::curve_reference::perpendicular_frame(
                    source.curve_ref(),
                    center,
                )?);
            if edge.is_none() && (!doc.is_selected(id) || key.eq_ignore_ascii_case("PickedCurve")) {
                release.push(id);
            }
            continue;
        }
        if word.contains(',') || input.base.is_none() && input.getter.slot() == Slot::First {
            let (point, n) = if word == "0" {
                (plane.origin(), 1)
            } else {
                crate::parse_point(&args[i..])?
            };
            input.point(point)?;
            i += n;
        } else {
            let value = viboceros_drafting::PointInput::parse_length_with_units(word, doc.units())
                .ok_or(CommandError::Usage(USAGE))?
                .map_err(|_| CommandError::Usage(USAGE))?;
            input.number(value)?;
            i += 1;
        }
    }
    if !input.complete() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((input, release))
}
