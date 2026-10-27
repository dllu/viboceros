//! Validate source continuity before rounded subcurve extraction.
use super::*;

pub(super) fn validate_rail_interval(
    rail: CurveRef<'_>,
    domain: RangeInclusive<Real>,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    // A rail kink needs a miter construction, not an interpolated jump of
    // transported profiles. Reject it until that topology is constructed.
    let start = *domain.start();
    let end = *domain.end();
    // A trim a few ULPs from a smooth repeated knot can round the tiny
    // remaining span to coincident controls. Test the original one-sided
    // tangents, restricting the original knot groups to this active interval.
    let source = rail.to_nurbs()?;
    let first = rail.nurbs_parameter(start)?;
    let last = rail.nurbs_parameter(end)?;
    for (knot, multiplicity) in source.interior_knot_groups() {
        if knot > first
            && knot < last
            && multiplicity == source.degree()
            && source.kink_angle_at(knot)? > tolerance.angular()
        {
            return Err(invalid("rail corners require sweep miter construction"));
        }
    }
    let trimmed = rail.to_owned().try_trimmed(domain)?;
    let closed_rail = trimmed.as_ref().is_closed()?;
    if closed_rail {
        let first = trimmed.as_ref().evaluate_with_tangent(start)?.tangent();
        let last = trimmed.as_ref().evaluate_with_tangent(end)?.tangent();
        if first.as_vector().angle_to(last.as_vector())? > tolerance.angular() {
            return Err(invalid(
                "rail seam corner requires sweep miter construction",
            ));
        }
    }
    Ok(closed_rail)
}
