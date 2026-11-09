//! Fixed-normal Hermite clocks retain spatial height and source span boundaries.
use super::*;
use crate::{AffineTransform3, CurveContinuity, ParameterSide, Plane, Point3, Vector3};

const MAX_DEPTH: usize = 4;
const CHECKS: usize = 7;
const MAX_PIECES: usize = 8192;
#[cfg(test)]
mod tests;

pub fn try_fixed_plane_ribbon(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    try_fixed_plane_ribbon_with_ruled(
        source,
        distance,
        both_sides,
        normal,
        tolerance,
        &mut NurbsSurface::try_ruled_from_curves,
    )
}

/// Fit regular fixed-normal loci using cubic Hermite pieces, with at most four
/// bisections per original span. Line-like projections translate the original
/// basis. The seven interior checks and depth cap reproduce the qualified
/// native cases, but do not certify constant distance or arbitrary-input parity.
pub fn try_fixed_plane_ribbon_with_ruled<E: From<GeometryError>>(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    tolerance: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    try_fixed_plane_ribbon_with_fit_tolerance(
        source, distance, both_sides, normal, tolerance, tolerance, ruled,
    )
}

/// Separate model sewing tolerance from the requested offset fit tolerance.
/// Near-parallel/perpendicular control planes have additional native switching
/// rules and are rejected here unless the projection is linear. Planar curves
/// parallel to the construction plane should use `try_smooth_ribbon` instead.
#[allow(clippy::too_many_arguments)]
pub fn try_fixed_plane_ribbon_with_fit_tolerance<E: From<GeometryError>>(
    source: &NurbsCurve,
    distance: Real,
    both_sides: bool,
    normal: UnitVector3,
    model: Tolerance,
    fit: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Brep, E> {
    build_with_offset(
        source,
        distance,
        both_sides,
        model,
        &mut |d| Ok(offset(source, d, normal, model, fit)?),
        ruled,
    )
}

fn offset(
    source: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
    fit: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    source.tight_bounds(tolerance)?;
    let projected = source.transformed(AffineTransform3::try_planar_projection(Plane::new(
        source.control_points()[0].point(),
        normal,
    ))?)?;
    if projected.is_linear(tolerance)? {
        let d = projected.domain();
        let direction = projected
            .evaluate(*d.start())?
            .vector_to(projected.evaluate(*d.end())?)?;
        let shift = normal
            .as_vector()
            .cross(direction)?
            .normalized_nonzero()?
            .as_vector()
            .scaled(distance)?;
        return source.transformed(AffineTransform3::from_translation(shift));
    }
    if fit.absolute() > tolerance.absolute()
        && (source.spans().count() > 1
            || source
                .control_points()
                .iter()
                .any(|p| p.weight() != source.control_points()[0].weight()))
    {
        // Coarse rational/multispan inputs can switch to a different native
        // seed and corner partition. Keep these explicit until reproduced.
        return Err(GeometryError::UnsupportedCurveOffset);
    }
    let controls = source
        .control_points()
        .iter()
        .map(|p| p.point())
        .collect::<Vec<_>>();
    let plane = crate::PointProjection3::onto_best_fit_plane(&controls)?.as_plane()?;
    let cosine = plane.normal().as_vector().dot(normal.as_vector())?.abs();
    // Deliberately conservative bounds around observed native mode switches.
    // These are support limits, not purported Rhino angular constants.
    if !(0.001..=0.9999).contains(&cosine) {
        return Err(GeometryError::UnsupportedCurveOffset);
    }
    if !source
        .discontinuity_parameters(CurveContinuity::G1, tolerance.angular())?
        .is_empty()
    {
        return Err(GeometryError::UnsupportedCurveOffset);
    }
    let spans = source.spans().collect::<Vec<_>>();
    if spans.len() > MAX_PIECES {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    let mut pending = spans
        .into_iter()
        .rev()
        .map(|(a, b)| (a, b, 0))
        .collect::<Vec<_>>();
    let mut points = Vec::new();
    let mut knots = vec![*source.domain().start(); 4];
    while let Some((a, b, depth)) = pending.pop() {
        let (p, v) = sample(source, distance, normal, a, ParameterSide::Right)?;
        let (q, w) = sample(source, distance, normal, b, ParameterSide::Left)?;
        let third = (b - a) / 3.;
        let controls = [
            p,
            p.translated(v.scaled(third)?)?,
            q.translated(w.scaled(-third)?)?,
            q,
        ];
        let mut accepted = depth == MAX_DEPTH;
        if !accepted {
            accepted = true;
            for check in 1..=CHECKS {
                let x = check as Real / (CHECKS + 1) as Real;
                let t = a.mul_add(1. - x, b * x);
                let (point, velocity) = source.evaluate_with_derivative(t)?;
                // This fitting target uses the cross product of the unit
                // spatial tangent. End jets instead normalize its projection.
                // A depth cap can therefore leave substantial locus residual.
                let expected = point.translated(
                    normal
                        .as_vector()
                        .cross(velocity.normalized_nonzero()?.as_vector())?
                        .scaled(distance)?,
                )?;
                if cubic(controls, x)?.distance_to(expected)? > fit.absolute() * 0.5 {
                    accepted = false;
                    break;
                }
            }
        }
        if !accepted {
            if points.len() / 3 + pending.len() + 2 > MAX_PIECES {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
            let middle = a.midpoint(b);
            if middle <= a || middle >= b {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
            pending.push((middle, b, depth + 1));
            pending.push((a, middle, depth + 1));
            continue;
        }
        if let Some(previous) = points.last() {
            if p.distance_to(*previous)? > tolerance.absolute() {
                return Err(GeometryError::UnsupportedCurveOffset);
            }
        } else {
            points.push(p);
        }
        points.extend_from_slice(&controls[1..]);
        knots.extend(std::iter::repeat_n(b, 3));
    }
    if source.is_closed()? {
        if points[0].distance_to(*points.last().unwrap())? > tolerance.absolute() {
            return Err(GeometryError::UnsupportedCurveOffset);
        }
        *points.last_mut().unwrap() = points[0];
    }
    knots.push(*source.domain().end());
    NurbsCurve::try_new(3, points, knots)
}

fn sample(
    source: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    t: Real,
    side: ParameterSide,
) -> Result<(Point3, Vector3), GeometryError> {
    let (p, v, a) = source.evaluate_with_second_derivative_on_side(t, side)?;
    let cross = normal.as_vector().cross(v)?;
    let length = cross.length()?;
    let left = cross.normalized_nonzero()?.as_vector();
    let derivative = normal.as_vector().cross(a)?.scaled(1. / length)?;
    let along = left.dot(derivative)?;
    let derivative = Vector3::try_new(
        derivative.x() - left.x() * along,
        derivative.y() - left.y() * along,
        derivative.z() - left.z() * along,
    )?;
    let tangent = Vector3::try_new(
        v.x() + distance * derivative.x(),
        v.y() + distance * derivative.y(),
        v.z() + distance * derivative.z(),
    )?;
    Ok((p.translated(left.scaled(distance)?)?, tangent))
}

fn cubic(points: [Point3; 4], x: Real) -> Result<Point3, GeometryError> {
    let lerp = |a: Point3, b: Point3| a.translated(a.vector_to(b)?.scaled(x)?);
    let a = lerp(points[0], points[1])?;
    let b = lerp(points[1], points[2])?;
    let c = lerp(points[2], points[3])?;
    lerp(lerp(a, b)?, lerp(b, c)?)
}
