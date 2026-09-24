//! Shared endpoint selection, orientation, and supporting-line intersection.

use crate::{
    Curve3, CurveSegment3, GeometryError, LineSegment, Point3, PolyCurve3, Tolerance, UnitVector3,
};

pub(super) fn original_direction(
    curve: Curve3,
    reverse: bool,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    if reverse {
        curve.reversed(tolerance)
    } else {
        Ok(curve)
    }
}

pub(super) fn curve_from_segments(segments: &[CurveSegment3]) -> Result<Curve3, GeometryError> {
    if segments.len() == 1 {
        Ok(segments[0].clone().into_curve())
    } else {
        Ok(Curve3::PolyCurve(PolyCurve3::try_new(segments.to_vec())?))
    }
}

pub(super) fn oriented(
    source: &Curve3,
    pick: Point3,
    pick_at_end: bool,
    tolerance: Tolerance,
) -> Result<PolyCurve3, GeometryError> {
    let selected_end = selected_end(source, pick, tolerance)?;
    if selected_end == pick_at_end {
        source.to_polycurve()
    } else {
        source.reversed(tolerance)?.to_polycurve()
    }
}

pub(super) fn selected_end(
    source: &Curve3,
    pick: Point3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if source.as_ref().is_closed()? {
        return Err(unsupported());
    }
    let start = source
        .as_ref()
        .evaluate(*source.as_ref().domain().start())?;
    let end = source.as_ref().evaluate(*source.as_ref().domain().end())?;
    let start_distance = pick.distance_to(start)?;
    let end_distance = pick.distance_to(end)?;
    if (start_distance - end_distance).abs() <= tolerance.absolute() {
        return Err(GeometryError::InvalidPolyCurve {
            context: "curve-end pick does not identify an end",
        });
    }
    Ok(end_distance < start_distance)
}

pub(super) fn meeting_lines(
    first: LineSegment,
    second: LineSegment,
    tolerance: Tolerance,
) -> Result<(LineSegment, LineSegment), GeometryError> {
    let meeting = supporting_line_intersection(first, second, tolerance)?;
    let first_direction = first.direction(tolerance)?.as_vector();
    let second_direction = second.direction(tolerance)?.as_vector();
    let first_distance = first.start().vector_to(meeting)?.dot(first_direction)?;
    let second_distance = second.start().vector_to(meeting)?.dot(second_direction)?;
    if first_distance <= tolerance.absolute()
        || second_distance >= second.length()? - tolerance.absolute()
    {
        return Err(unsupported());
    }
    Ok((
        LineSegment::try_new(first.start(), meeting, Tolerance::NUMERICAL_VALIDATION)?,
        LineSegment::try_new(meeting, second.end(), Tolerance::NUMERICAL_VALIDATION)?,
    ))
}

/// Intersection of the infinite supports of nonparallel, coplanar lines.
pub(super) fn supporting_line_intersection(
    first: LineSegment,
    second: LineSegment,
    tolerance: Tolerance,
) -> Result<Point3, GeometryError> {
    supporting_directions_intersection(
        first.start(),
        first.direction(tolerance)?,
        second.start(),
        second.direction(tolerance)?,
        tolerance,
    )
}

/// Intersection from points and unit directions, avoiding synthetic short
/// segments that lose their tangent at large model coordinates.
pub(super) fn supporting_directions_intersection(
    first_start: Point3,
    first_unit: UnitVector3,
    second_start: Point3,
    second_unit: UnitVector3,
    tolerance: Tolerance,
) -> Result<Point3, GeometryError> {
    let first_direction = first_unit.as_vector();
    let second_direction = second_unit.as_vector();
    let normal = first_direction.cross(second_direction)?;
    let denominator = normal.dot(normal)?;
    if denominator <= tolerance.angular().sin().powi(2) {
        return Err(unsupported());
    }
    let between = first_start.vector_to(second_start)?;
    let first_distance = between.cross(second_direction)?.dot(normal)? / denominator;
    let second_distance = between.cross(first_direction)?.dot(normal)? / denominator;
    let first_meeting = first_start.translated(first_direction.scaled(first_distance)?)?;
    let second_meeting = second_start.translated(second_direction.scaled(second_distance)?)?;
    if first_meeting.distance_to(second_meeting)? > tolerance.absolute() {
        return Err(unsupported());
    }
    first_meeting.midpoint(second_meeting)
}

pub(super) fn unsupported() -> GeometryError {
    GeometryError::InvalidPolyCurve {
        context: "selected curve ends cannot form a supported corner operation",
    }
}
