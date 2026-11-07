//! Straight chamfers between selected ends of open curves.

use crate::{
    Curve3, CurveArcExtensionStyle, CurveExtensionOperations, CurveOtherExtensionStyle, CurveRef,
    CurveSegment3, GeometryError, LineSegment, Point3, PolyCurve3, Real, Tolerance,
    curve_connect_pair::connected_ends_with_operations,
    curve_pair_support::{
        curve_from_segments, meeting_lines, oriented, original_direction, selected_end,
    },
    nurbs::with_portable_extension_operations,
};

mod connect;
mod setback;

/// Styles used to extend nonmeeting curve ends before chamfering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurveChamferExtensionStyles {
    pub arc: CurveArcExtensionStyle,
    pub other: CurveOtherExtensionStyle,
}

impl Default for CurveChamferExtensionStyles {
    fn default() -> Self {
        Self {
            arc: CurveArcExtensionStyle::Arc,
            other: CurveOtherExtensionStyle::Line,
        }
    }
}

/// Connects the selected curve ends using the requested extension styles,
/// then finds each Euclidean setback from their intersection.
pub fn try_chamfer_curves_joined_with_styles(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    with_portable_extension_operations(|operations| {
        try_chamfer_curves_joined_with_operations(
            (first, first_pick),
            (second, second_pick),
            distances,
            styles,
            tolerance,
            operations,
        )
    })
}

/// Shared connected-end policy with delegated NURBS extrapolation and setback extraction.
pub fn try_chamfer_curves_joined_with_operations<E: From<GeometryError>>(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [Real; 2],
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
    operations: &mut CurveExtensionOperations<'_, E>,
) -> Result<PolyCurve3, E> {
    let (connected, picks) = connect::connected(first, second, styles, tolerance, operations)?;
    Ok(chamfer_connected_curves(
        &connected[0],
        picks[0],
        &connected[1],
        picks[1],
        distances,
        tolerance,
        operations,
    )?
    .0)
}

/// Creates independent retained curves and a chamfer with extension options.
pub fn try_chamfer_curves_parts_with_styles(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [Real; 2],
    trim: bool,
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    with_portable_extension_operations(|operations| {
        try_chamfer_curves_parts_with_operations(
            first, second, distances, trim, styles, tolerance, operations,
        )
    })
}

/// Shared connected-end policy with delegated NURBS extrapolation and setback extraction.
pub fn try_chamfer_curves_parts_with_operations<E: From<GeometryError>>(
    first: (&Curve3, Point3),
    second: (&Curve3, Point3),
    distances: [Real; 2],
    trim: bool,
    styles: CurveChamferExtensionStyles,
    tolerance: Tolerance,
    operations: &mut CurveExtensionOperations<'_, E>,
) -> Result<Vec<Curve3>, E> {
    let (connected, picks) = connect::connected(first, second, styles, tolerance, operations)?;
    let reverse_first = !selected_end(&connected[0], picks[0], tolerance)?;
    let reverse_second = selected_end(&connected[1], picks[1], tolerance)?;
    let (joined, first_count, has_bevel) = chamfer_connected_curves(
        &connected[0],
        picks[0],
        &connected[1],
        picks[1],
        distances,
        tolerance,
        operations,
    )?;
    chamfer_parts_from_joined(
        &joined,
        first_count,
        has_bevel,
        trim,
        reverse_first,
        reverse_second,
        tolerance,
    )
    .map_err(E::from)
}

fn chamfer_connected_curves<E: From<GeometryError>>(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    tolerance: Tolerance,
    operations: &mut CurveExtensionOperations<'_, E>,
) -> Result<(PolyCurve3, usize, bool), E> {
    validate_distances(distances[0], distances[1])?;
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let first_parameter =
        setback::parameter(CurveRef::PolyCurve(&first), distances[0], true, tolerance)?;
    let second_parameter =
        setback::parameter(CurveRef::PolyCurve(&second), distances[1], false, tolerance)?;
    let first_retained = first.try_trimmed_with_nurbs(
        *first.domain().start()..=first_parameter,
        &mut |curve, interval| (operations.trim)(curve, interval),
    )?;
    let second_retained = second.try_trimmed_with_nurbs(
        second_parameter..=*second.domain().end(),
        &mut |curve, interval| (operations.trim)(curve, interval),
    )?;
    let first_cut = first_retained.evaluate(*first_retained.domain().end())?;
    let second_cut = second_retained.evaluate(*second_retained.domain().start())?;
    let first_count = first_retained.segments().len();
    let has_bevel = first_cut.distance_to(second_cut)? > tolerance.absolute();
    let mut segments = first_retained.segments().to_vec();
    if has_bevel {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            first_cut,
            second_cut,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    segments.extend_from_slice(second_retained.segments());
    Ok((PolyCurve3::try_new(segments)?, first_count, has_bevel))
}

fn chamfer_parts_from_joined(
    joined: &PolyCurve3,
    first_count: usize,
    has_bevel: bool,
    trim: bool,
    reverse_first: bool,
    reverse_second: bool,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    if !trim && !has_bevel {
        return Err(unsupported());
    }
    let segments = joined.segments();
    if !trim {
        return Ok(vec![segments[first_count].clone().into_curve()]);
    }
    let second_start = first_count + usize::from(has_bevel);
    let mut parts = vec![
        original_direction(
            curve_from_segments(&segments[..first_count])?,
            reverse_first,
            tolerance,
        )?,
        original_direction(
            curve_from_segments(&segments[second_start..])?,
            reverse_second,
            tolerance,
        )?,
    ];
    if has_bevel {
        parts.push(segments[first_count].clone().into_curve());
    }
    Ok(parts)
}

/// Trims or extends terminal straight segments to a bevel whose setbacks are
/// measured along the two supporting lines from their intersection.
pub fn try_chamfer_curves_joined(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    first_distance: Real,
    second_distance: Real,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    validate_distances(first_distance, second_distance)?;
    let first = oriented(first, first_pick, true, tolerance)?;
    let second = oriented(second, second_pick, false, tolerance)?;
    let last = first.segments().len() - 1;
    let first_terminal = &first.segments()[last];
    let second_terminal = &second.segments()[0];
    let (first_retained, second_retained, first_cut, second_cut) =
        if let (CurveSegment3::Line(first_line), CurveSegment3::Line(second_line)) =
            (first_terminal, second_terminal)
        {
            let (first_line, second_line) = meeting_lines(*first_line, *second_line, tolerance)?;
            let corner = first_line.end();
            let first_direction = first_line.direction(tolerance)?.as_vector();
            let second_direction = second_line.direction(tolerance)?.as_vector();
            let first_cut = corner.translated(first_direction.scaled(-first_distance)?)?;
            let second_cut = corner.translated(second_direction.scaled(second_distance)?)?;
            if first_distance >= first_line.length()? - tolerance.absolute()
                || second_distance >= second_line.length()? - tolerance.absolute()
            {
                return Err(unsupported());
            }
            (
                CurveSegment3::Line(LineSegment::try_new(
                    first_line.start(),
                    first_cut,
                    Tolerance::NUMERICAL_VALIDATION,
                )?),
                CurveSegment3::Line(LineSegment::try_new(
                    second_cut,
                    second_line.end(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?),
                first_cut,
                second_cut,
            )
        } else {
            let end = first_terminal.evaluate(*first_terminal.domain().end())?;
            let start = second_terminal.evaluate(*second_terminal.domain().start())?;
            if end.distance_to(start)? > tolerance.absolute() {
                return Err(unsupported());
            }
            let first_parameter =
                setback::parameter(first_terminal.as_ref(), first_distance, true, tolerance)?;
            let second_parameter =
                setback::parameter(second_terminal.as_ref(), second_distance, false, tolerance)?;
            let first_retained =
                first_terminal.try_trimmed(*first_terminal.domain().start()..=first_parameter)?;
            let second_retained =
                second_terminal.try_trimmed(second_parameter..=*second_terminal.domain().end())?;
            let first_cut = first_retained.evaluate(*first_retained.domain().end())?;
            let second_cut = second_retained.evaluate(*second_retained.domain().start())?;
            (first_retained, second_retained, first_cut, second_cut)
        };
    let mut segments = first.segments()[..last].to_vec();
    segments.push(first_retained);
    if first_cut.distance_to(second_cut)? > tolerance.absolute() {
        segments.push(CurveSegment3::Line(LineSegment::try_new(
            first_cut,
            second_cut,
            Tolerance::NUMERICAL_VALIDATION,
        )?));
    }
    segments.push(second_retained);
    segments.extend_from_slice(&second.segments()[1..]);
    PolyCurve3::try_new(segments)
}

/// Returns retained source curves in their original directions, followed by
/// the bevel. With trimming off, only the bevel is returned.
pub fn try_chamfer_curves_parts(
    first: &Curve3,
    first_pick: Point3,
    second: &Curve3,
    second_pick: Point3,
    distances: [Real; 2],
    trim: bool,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    if !trim && distances == [0.0, 0.0] {
        return Err(unsupported());
    }
    let reverse_first = !selected_end(first, first_pick, tolerance)?;
    let reverse_second = selected_end(second, second_pick, tolerance)?;
    let first_count = first.to_polycurve()?.segments().len();
    let joined = try_chamfer_curves_joined(
        first,
        first_pick,
        second,
        second_pick,
        distances[0],
        distances[1],
        tolerance,
    )?;
    let second_count = second.to_polycurve()?.segments().len();
    let has_bevel = joined.segments().len() > first_count + second_count;
    chamfer_parts_from_joined(
        &joined,
        first_count,
        has_bevel,
        trim,
        reverse_first,
        reverse_second,
        tolerance,
    )
}

fn validate_distances(first: Real, second: Real) -> Result<(), GeometryError> {
    if !first.is_finite() || !second.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "curve chamfer distance",
        });
    }
    if first < 0.0 || second < 0.0 {
        return Err(GeometryError::Degenerate {
            context: "curve chamfer distance",
        });
    }
    Ok(())
}

fn unsupported() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "selected curve ends cannot form a supported chamfer",
    }
}

#[cfg(test)]
mod tests;
