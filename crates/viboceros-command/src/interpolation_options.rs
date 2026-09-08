//! Shared InterpCrv parsing for complete commands and atomic draft updates.

use super::{CommandError, GeometryError, INTERP_CRV_USAGE, option_name_eq, parse_point};
use viboceros_geometry::{
    CurveInterpolationOptions, CurveKnotSpacing, InterpolatedCurveClosure, Point3, Vector3,
};

/// Parses an options-only InterpCrv tail for interactive drafting. Geometry
/// validation remains with the interpolation constructor at completion.
pub fn parse_interp_curve_options(
    arguments: &[&str],
) -> Result<CurveInterpolationOptions, CommandError> {
    update_interp_curve_options(CurveInterpolationOptions::default(), arguments)
}

/// Applies an options-only update atomically, retaining unspecified settings.
pub fn update_interp_curve_options(
    current: CurveInterpolationOptions,
    arguments: &[&str],
) -> Result<CurveInterpolationOptions, CommandError> {
    let (points, options) = parse_interp_curve_arguments_from(arguments, current)?;
    if !points.is_empty() {
        return Err(CommandError::Usage(INTERP_CRV_USAGE));
    }
    if !matches!(options.degree(), 1 | 3) {
        return Err(GeometryError::UnsupportedCurveInterpolationDegree {
            actual: options.degree(),
        }
        .into());
    }
    if (options.start_tangent().is_some() || options.end_tangent().is_some())
        && (options.degree() != 3 || options.closure() != InterpolatedCurveClosure::Open)
    {
        return Err(GeometryError::CurveInterpolationTangentsRequireOpenCubic.into());
    }
    // Public callers can construct options directly, bypassing token parsing.
    // Validate retained directions as well as newly supplied ones.
    for tangent in [options.start_tangent(), options.end_tangent()]
        .into_iter()
        .flatten()
    {
        tangent.normalized_nonzero()?;
    }
    Ok(options)
}

pub(super) fn parse_interp_curve_arguments(
    arguments: &[&str],
) -> Result<(Vec<Point3>, CurveInterpolationOptions), CommandError> {
    parse_interp_curve_arguments_from(arguments, CurveInterpolationOptions::default())
}

fn parse_interp_curve_arguments_from(
    arguments: &[&str],
    current: CurveInterpolationOptions,
) -> Result<(Vec<Point3>, CurveInterpolationOptions), CommandError> {
    let mut points = Vec::new();
    let mut degree = current.degree();
    let mut knot_spacing = current.knot_spacing();
    let mut closure = current.closure();
    let mut start_tangent = current.start_tangent();
    let mut end_tangent = current.end_tangent();
    let mut degree_seen = false;
    let mut knots_seen = false;
    let mut close_seen = false;
    let mut start_tangent_seen = false;
    let mut end_tangent_seen = false;
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index];
        if let Some((name, value)) = argument.split_once('=') {
            let value = value.trim_start_matches('_');
            if option_name_eq(name, "Degree") && !degree_seen {
                degree = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::InvalidInteger(value.to_owned()))?;
                degree_seen = true;
            } else if option_name_eq(name, "Knots") && !knots_seen {
                knot_spacing = if value.eq_ignore_ascii_case("Uniform") {
                    CurveKnotSpacing::Uniform
                } else if value.eq_ignore_ascii_case("Chord") {
                    CurveKnotSpacing::Chord
                } else if value.eq_ignore_ascii_case("SqrtChrd")
                    || value.eq_ignore_ascii_case("SqrtChord")
                    || value.eq_ignore_ascii_case("ChordSquareRoot")
                    || value.eq_ignore_ascii_case("SquareRootChord")
                {
                    CurveKnotSpacing::SquareRootChord
                } else {
                    return Err(CommandError::Usage(INTERP_CRV_USAGE));
                };
                knots_seen = true;
            } else if option_name_eq(name, "Close") && !close_seen {
                closure = if value.eq_ignore_ascii_case("Open") || value.eq_ignore_ascii_case("No")
                {
                    InterpolatedCurveClosure::Open
                } else if value.eq_ignore_ascii_case("Smooth") || value.eq_ignore_ascii_case("Yes")
                {
                    InterpolatedCurveClosure::Smooth
                } else if value.eq_ignore_ascii_case("Sharp") {
                    InterpolatedCurveClosure::Sharp
                } else {
                    return Err(CommandError::Usage(INTERP_CRV_USAGE));
                };
                close_seen = true;
            } else if option_name_eq(name, "StartTangent") && !start_tangent_seen {
                start_tangent = parse_interp_curve_tangent(value)?;
                start_tangent_seen = true;
            } else if option_name_eq(name, "EndTangent") && !end_tangent_seen {
                end_tangent = parse_interp_curve_tangent(value)?;
                end_tangent_seen = true;
            } else {
                return Err(CommandError::Usage(INTERP_CRV_USAGE));
            }
            index += 1;
        } else {
            let (point, consumed) = parse_point(&arguments[index..])?;
            points.push(point);
            index += consumed;
        }
    }

    let mut options = CurveInterpolationOptions::new(degree, knot_spacing, closure);
    if let Some(tangent) = start_tangent {
        options = options.with_start_tangent(tangent);
    }
    if let Some(tangent) = end_tangent {
        options = options.with_end_tangent(tangent);
    }
    Ok((points, options))
}

fn parse_interp_curve_tangent(value: &str) -> Result<Option<Vector3>, CommandError> {
    if value.eq_ignore_ascii_case("None") {
        return Ok(None);
    }
    if !value.contains(',') {
        return Err(CommandError::Usage(INTERP_CRV_USAGE));
    }
    let (point, consumed) = parse_point(&[value])?;
    debug_assert_eq!(consumed, 1);
    let tangent = Vector3::try_from(point.to_array())?;
    // Match interpolation's direction validation at entry, but retain the
    // user's vector rather than replacing it with a rounded unit vector.
    tangent.normalized_nonzero()?;
    Ok(Some(tangent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_validate_retained_options_and_allow_explicit_repairs() {
        let zero = Vector3::try_new(0., 0., 0.).unwrap();
        for current in [
            CurveInterpolationOptions::default().with_start_tangent(zero),
            CurveInterpolationOptions::default().with_end_tangent(zero),
        ] {
            assert!(update_interp_curve_options(current, &[]).is_err());
            assert!(update_interp_curve_options(current, &["Knots=Uniform"]).is_err());
            let repaired =
                update_interp_curve_options(current, &["StartTangent=None", "EndTangent=None"])
                    .unwrap();
            assert_eq!(repaired, CurveInterpolationOptions::default());
        }
        let unsupported = CurveInterpolationOptions::new(
            2,
            CurveKnotSpacing::Uniform,
            InterpolatedCurveClosure::Open,
        );
        assert!(update_interp_curve_options(unsupported, &[]).is_err());
        let repaired = update_interp_curve_options(unsupported, &["Degree=3"]).unwrap();
        assert_eq!(repaired.degree(), 3);
        assert_eq!(repaired.knot_spacing(), CurveKnotSpacing::Uniform);
    }

    #[test]
    fn updates_preserve_valid_extreme_directions_without_normalizing_stored_values() {
        for scale in [f64::from_bits(1), 1., f64::MAX] {
            let direction = Vector3::try_new(scale, -scale, scale).unwrap();
            let current = CurveInterpolationOptions::default()
                .with_start_tangent(direction)
                .with_end_tangent(direction);
            let updated = update_interp_curve_options(current, &["Knots=Uniform"]).unwrap();
            assert_eq!(updated.start_tangent(), Some(direction));
            assert_eq!(updated.end_tangent(), Some(direction));
            assert_eq!(updated.knot_spacing(), CurveKnotSpacing::Uniform);
        }
    }

    #[test]
    fn option_updates_do_not_accept_points_unknown_options_or_duplicates() {
        let current = CurveInterpolationOptions::default();
        for arguments in [
            vec!["0,0,0"],
            vec!["Unknown=Yes"],
            vec!["Degree=3", "Degree=1"],
            vec!["Knots=Uniform", "0,0,0"],
            vec!["StartTangent=None", "StartTangent=1,0,0"],
        ] {
            assert!(
                update_interp_curve_options(current, &arguments).is_err(),
                "{arguments:?}"
            );
        }
        assert_eq!(update_interp_curve_options(current, &[]).unwrap(), current);
    }
}
