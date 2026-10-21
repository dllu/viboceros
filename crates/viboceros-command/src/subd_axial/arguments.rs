//! Complete script parsing uses the same staged input as the viewport.
use super::*;
pub fn from_arguments_for(
    kind: Kind,
    doc: &Document,
    args: &[&str],
    plane: Frame3,
    preferences: Preferences,
) -> Result<Input, CommandError> {
    let mut input = Input::new_for(kind, plane, preferences);
    let mut i = 0;
    while i < args.len() {
        if input.definition.is_some() {
            return Err(CommandError::Usage(kind.usage()));
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
                    if tangent.has_curve_contacts() {
                        input.tangent_curve_circle(circle)?;
                    } else {
                        input.reference_circle(circle)?;
                    }
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
        return Err(CommandError::Usage(kind.usage()));
    }
    Ok(input)
}
