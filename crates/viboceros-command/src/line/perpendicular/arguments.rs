use super::*;
use crate::{CommandContext, parse_finite_real, parse_point};
use viboceros_document::Geometry;
pub fn requested(args: &[&str]) -> bool {
    args.iter()
        .take_while(|s| !s.contains(',') && !s.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .any(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Perpendicular")
        })
}
pub(crate) fn run(
    doc: &mut Document,
    args: &[&str],
    context: CommandContext,
) -> Result<String, CommandError> {
    let mut input = Input::new(context.construction_plane, doc.tolerance());
    let mut i = 0;
    let mut result = None;
    while i < args.len() {
        if result.is_some() {
            return Err(CommandError::Usage(super::super::USAGE));
        }
        if args[i]
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Perpendicular")
            || input.option(args[i])?
        {
            i += 1;
            continue;
        }
        if args[i]
            .trim_start_matches('_')
            .eq_ignore_ascii_case("Enter")
        {
            if !input.enter() {
                return Err(CommandError::Usage("Line Perpendicular cancelled"));
            }
            i += 1;
            continue;
        }
        if let Some((key, value)) = args[i].trim_start_matches('_').split_once('=')
            && key.eq_ignore_ascii_case("Planar")
        {
            input.set_drafting(
                context.construction_plane,
                crate::parse_yes_no(value).ok_or(CommandError::Usage("Planar=Yes|No"))?,
            );
            i += 1;
            continue;
        }
        if let Some((id, component)) = super::super::normal::parse_binding(args[i]) {
            let edge = match component {
                super::super::normal::Component::Object => None,
                super::super::normal::Component::Edge(e) => Some(e),
                _ => {
                    return Err(CommandError::Usage(
                        "Perpendicular Curve=id|Edge=id,index pick",
                    ));
                }
            };
            let (point, n) = parse_point(&args[i + 1..])?;
            result = input.pick_curve(doc, id, edge, point)?;
            i += 1 + n;
        } else if input.endpoint()
            && !args[i].contains(',')
            && (args.len() - i < 3
                || args[i + 1].parse::<f64>().is_err()
                || args[i + 2].parse::<f64>().is_err())
        {
            input.number(parse_finite_real(args[i])?)?;
            i += 1;
        } else {
            let (point, n) = parse_point(&args[i..])?;
            result = input.point(point)?;
            i += n;
        }
    }
    let definition = result.ok_or(CommandError::Usage(
        "Line Perpendicular [BothSides] Curve=id pick end",
    ))?;
    if input.clears_selection() {
        doc.clear_selection();
    }
    let length = definition.line.length()?;
    let id = doc.add_geometry(Geometry::Line(definition.line))?;
    Ok(format!("Added line {id} (length {length:.6})"))
}
