//! Strict options for fixed-orientation curve-path extrusion.
use super::*;

pub(super) fn parse(
    arguments: &[&str],
) -> Result<(Option<String>, bool, bool, bool), CommandError> {
    let mut path_name = None;
    let mut delete_input = None;
    let mut output_seen = false;
    let mut solid = None;
    let mut split_at_tangents = None;
    for argument in arguments {
        let Some((name, value)) = argument.split_once('=') else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE));
        };
        if option_name_eq(name, "PathName") && path_name.is_none() && !value.is_empty() {
            path_name = Some(value.to_owned());
        } else if option_name_eq(name, "DeleteInput") && delete_input.is_none() {
            delete_input = Some(
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?,
            );
        } else if option_name_eq(name, "Output") && !output_seen {
            if !value
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Surface")
            {
                return Err(CommandError::UnsupportedCurveAlongCurveOutput);
            }
            output_seen = true;
        } else if option_name_eq(name, "Solid") && solid.is_none() {
            solid = Some(
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?,
            );
        } else if option_name_eq(name, "SplitAtTangents") && split_at_tangents.is_none() {
            split_at_tangents = Some(
                parse_yes_no(value).ok_or(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE))?,
            );
        } else {
            return Err(CommandError::Usage(EXTRUDE_CURVE_ALONG_CURVE_USAGE));
        }
    }
    Ok((
        path_name,
        delete_input.unwrap_or(false),
        solid.unwrap_or(false),
        split_at_tangents.unwrap_or(false),
    ))
}
