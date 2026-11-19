//! Signed normal section for both Ribbon point getters.
use crate::parameter::map_parameter;
use crate::point::PointDistance;
use crate::{
    GeometryError, LineSegment, NurbsCurve, ParameterSide, Point3, Real, Tolerance, UnitVector3,
};

/// The signed perpendicular distance at the closest source location. At a
/// corner, select the larger absolute distance from the two adjacent sections.
pub fn ribbon_side_distance(
    curve: &NurbsCurve,
    point: Point3,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let normal = crate::curve_offset::nurbs_offset_plane(curve, normal, tolerance)?;
    let parameter = match linear_closest_parameter(curve, point, tolerance) {
        Some(parameter) => parameter,
        None => curve.closest_parameter(point, tolerance)?,
    };
    let domain = curve.domain();
    let at_seam =
        (parameter == *domain.start() || parameter == *domain.end()) && curve.is_closed()?;
    let distance = |parameter, side| {
        let (nearest, tangent) = curve.evaluate_with_derivative_on_side(parameter, side)?;
        let left = normal.as_vector().cross(tangent)?.normalized_nonzero()?;
        nearest.vector_to(point)?.dot(left.as_vector())
    };
    let (before, after) = if at_seam {
        (*domain.end(), *domain.start())
    } else {
        (parameter, parameter)
    };
    let left = distance(before, ParameterSide::Left)?;
    let right = distance(after, ParameterSide::Right)?;
    Ok(if left.abs() > right.abs() {
        left
    } else {
        right
    })
}

/// Width inferred by the ThroughPoint getter. A nonzero pick can describe a
/// ribbon thinner than the document tolerance.
pub fn ribbon_through_distance(
    curve: &NurbsCurve,
    point: Point3,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let distance = ribbon_side_distance(curve, point, normal, tolerance)?;
    if distance == 0.0 {
        return Err(GeometryError::InvalidCurveOffsetDistance);
    }
    Ok(distance)
}

// Linear, uniform-weight spans can be projected directly in a single pass.
// This keeps the side preview cheap for dense polylines.
fn linear_closest_parameter(
    curve: &NurbsCurve,
    point: Point3,
    tolerance: Tolerance,
) -> Option<Real> {
    let controls = curve.control_points();
    if curve.degree() != 1 || controls.iter().any(|p| p.weight() != controls[0].weight()) {
        return None;
    }
    let mut nearest: Option<(PointDistance, Real)> = None;
    for (index, controls) in controls.windows(2).enumerate() {
        let a = curve.knots()[index + 1];
        let b = curve.knots()[index + 2];
        if a == b {
            return None;
        }
        let line = LineSegment::from_validated(controls[0].point(), controls[1].point(), [a, b]);
        let fraction = line.closest_parameter(point, tolerance).ok()?;
        let distance = PointDistance::new(point, line.point_at(fraction).ok()?);
        if nearest
            .as_ref()
            .is_none_or(|(best, _)| distance.compare(best, point).is_lt())
        {
            let parameter = map_parameter(fraction, 0.0..=1.0, a..=b).ok()?;
            nearest = Some((distance, parameter));
        }
    }
    nearest.map(|(_, parameter)| parameter)
}
