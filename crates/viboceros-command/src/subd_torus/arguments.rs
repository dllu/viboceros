//! Complete script parsing uses the same staged input as the viewport.
use super::*;
pub fn from_arguments(
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    preferences: Preferences,
) -> Result<Input, CommandError> {
    let mut input = Input::new(plane, preferences);
    let mut i = 0;
    while i < args.len() {
        if input.definition.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let word = args[i];
        if input.at_start()
            && word
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Tangent")
        {
            let mut tangent = crate::circle_tangent_input::Input::new(
                plane,
                doc.tolerance(),
                crate::circle_around_input::Preferences {
                    radius: input.preferences.radius,
                    ..Default::default()
                },
            );
            i += 1;
            loop {
                let (n, circle) = tangent.argument(doc, &args[i..])?;
                i += n;
                if let Some(circle) = circle {
                    input.reference_circle(circle)?;
                    break;
                }
            }
            continue;
        }
        if input.at_start()
            && word
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("FitPoints")
        {
            i += 1;
            input.fit_points(doc)?;
            continue;
        }
        if input.at_start()
            && word
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("AroundCurve")
        {
            i += 1;
            let word = args.get(i).ok_or(CommandError::Usage(USAGE))?;
            let (key, value) = word
                .trim_start_matches(['_', '-'])
                .split_once('=')
                .ok_or(CommandError::Usage(USAGE))?;
            let (id, edge) = if key.eq_ignore_ascii_case("Curve") {
                (value.parse().map_err(|_| CommandError::Usage(USAGE))?, None)
            } else if key.eq_ignore_ascii_case("Edge") {
                let (id, edge) =
                    crate::curve_reference::parse_edge(value).ok_or(CommandError::Usage(USAGE))?;
                (id, Some(edge))
            } else {
                return Err(CommandError::Usage(USAGE));
            };
            let source = crate::curve_reference::CapturedCurve::capture(doc, id, edge)?;
            i += 1;
            let (center, n) = crate::parse_point(&args[i..])?;
            i += n;
            input.around_frame(crate::curve_reference::perpendicular_frame(
                source.curve_ref(),
                center,
            )?);
            continue;
        }
        if input.option(word)? {
            i += 1;
            continue;
        }
        if word.trim_start_matches('_').eq_ignore_ascii_case("Enter") {
            input.enter()?;
            i += 1;
            continue;
        }
        if word.contains(',') || !input.wants_number() {
            let (p, n) = if word == "0" && !input.wants_number() {
                (plane.origin(), 1)
            } else {
                crate::parse_point(&args[i..])?
            };
            input.point(p)?;
            i += n;
        } else {
            input.number(crate::parse_finite_real(word)?)?;
            i += 1;
        }
    }
    if input.definition.is_none() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(input)
}
