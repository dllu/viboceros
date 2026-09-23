//! Shared directed partial-curve input for read-only measurements.
use super::*;
use crate::{option_name_eq, orient_option, parse_finite_real, parse_point};
use viboceros_geometry::{Curve3, CurveRef};

pub(super) fn parse_subcurve(
    curve: CurveRef<'_>,
    tolerance: Tolerance,
    arguments: &[&str],
    usage: &'static str,
) -> Result<Curve3, CommandError> {
    let Some(first) = arguments.first() else {
        return Err(CommandError::Usage(usage));
    };
    let option = first.split_once('=').map_or(*first, |(name, _)| name);
    let source = curve.to_owned();
    let [start, end] = if option_name_eq(option, "Parameter") {
        let (name, value, consumed) = orient_option(arguments, 0, usage)?;
        if !option_name_eq(name, "Parameter") {
            return Err(CommandError::Usage(usage));
        }
        require_consumed(arguments, consumed, usage)?;
        let (start, end) = value.split_once(',').ok_or(CommandError::Usage(usage))?;
        if start.is_empty() || end.is_empty() || end.contains(',') {
            return Err(CommandError::Usage(usage));
        }
        [parse_finite_real(start)?, parse_finite_real(end)?]
    } else {
        let (start_point, consumed) = parse_point(arguments)?;
        let (end_point, second_consumed) = parse_point(&arguments[consumed..])?;
        require_consumed(arguments, consumed + second_consumed, usage)?;
        [
            source.as_ref().closest_parameter(start_point, tolerance)?,
            source.as_ref().closest_parameter(end_point, tolerance)?,
        ]
    };
    Ok(source.try_subcurve(start, end)?)
}
