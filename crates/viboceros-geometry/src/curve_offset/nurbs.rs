//! Tolerance-checked cubic approximations of smooth planar NURBS offsets.

use crate::{
    GeometryError, NurbsCurve, ParameterSide, Point3, Real, Tolerance, UnitVector3, Vector3,
};

const MAX_OFFSET_SPANS: usize = 8_192;
const CHECKS_PER_SPAN: usize = 15;

#[derive(Clone, Copy)]
struct Sample {
    parameter: Real,
    point: Point3,
    tangent: Vector3,
}

pub(super) fn offset_plane(
    curve: &NurbsCurve,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<UnitVector3, GeometryError> {
    let origin = curve.control_points()[0].point();
    let displacements = curve
        .control_points()
        .iter()
        .map(|control| origin.vector_to(control.point()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut scale = 1.0_f64;
    let mut longest = 0.0_f64;
    let mut axis = Vector3::try_new(0.0, 0.0, 0.0)?;
    for displacement in &displacements {
        let length = displacement.length()?;
        scale = scale.max(length);
        if length > longest {
            longest = length;
            axis = *displacement;
        }
    }
    let mut best_cross = Vector3::try_new(0.0, 0.0, 0.0)?;
    let mut best_area = 0.0;
    for displacement in &displacements {
        let cross = axis.cross(*displacement)?;
        let area = cross.length()?;
        if area > best_area {
            best_area = area;
            best_cross = cross;
        }
    }
    let mut normal = if best_area > tolerance.absolute() * scale {
        best_cross.normalized_nonzero()?
    } else if longest > tolerance.absolute() {
        axis.cross(fallback.as_vector())?
            .cross(axis)?
            .normalized_nonzero()?
    } else {
        fallback
    };
    if normal.as_vector().dot(fallback.as_vector())? < 0.0 {
        normal = normal.as_vector().scaled(-1.0)?.normalized_nonzero()?;
    }
    // Every finite rational span stays in the affine hull of its controls.
    for displacement in displacements {
        if displacement.dot(normal.as_vector())?.abs()
            > tolerance.absolute().max(tolerance.relative() * scale)
        {
            return Err(GeometryError::NonPlanarCurveOffset);
        }
    }
    Ok(normal)
}

fn offset_sample(
    curve: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    parameter: Real,
    side: ParameterSide,
) -> Result<Sample, GeometryError> {
    let (point, velocity, acceleration) =
        curve.evaluate_with_second_derivative_on_side(parameter, side)?;
    let speed = velocity.length()?;
    if speed == 0.0 {
        return Err(GeometryError::Degenerate {
            context: "NURBS offset stationary point",
        });
    }
    let left = normal.as_vector().cross(velocity)?.normalized_nonzero()?;
    let curvature = velocity.cross(acceleration)?.dot(normal.as_vector())? / speed.powi(3);
    let factor = 1.0 - distance * curvature;
    if !factor.is_finite() || factor <= 0.0 {
        return Err(GeometryError::Degenerate {
            context: "NURBS offset cusp",
        });
    }
    Ok(Sample {
        parameter,
        point: point.translated(left.as_vector().scaled(distance)?)?,
        tangent: velocity.scaled(factor)?,
    })
}

fn cubic_controls(start: Sample, end: Sample) -> Result<[Point3; 4], GeometryError> {
    let third = (end.parameter - start.parameter) / 3.0;
    Ok([
        start.point,
        start.point.translated(start.tangent.scaled(third)?)?,
        end.point.translated(end.tangent.scaled(-third)?)?,
        end.point,
    ])
}

fn cubic_point(controls: [Point3; 4], fraction: Real) -> Result<Point3, GeometryError> {
    let interpolate = |a: Point3, b: Point3| -> Result<Point3, GeometryError> {
        a.translated(a.vector_to(b)?.scaled(fraction)?)
    };
    let a = interpolate(controls[0], controls[1])?;
    let b = interpolate(controls[1], controls[2])?;
    let c = interpolate(controls[2], controls[3])?;
    interpolate(interpolate(a, b)?, interpolate(b, c)?)
}

pub(super) fn offset_nurbs(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let normal = offset_plane(curve, fallback, tolerance)?;
    curve.tight_bounds(tolerance)?; // Reject poles before the adaptive fit.
    let source_spans = curve.spans().collect::<Vec<_>>();
    if source_spans.len() > MAX_OFFSET_SPANS {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    let mut boundaries = Vec::with_capacity(source_spans.len());
    for &(a, b) in &source_spans {
        boundaries.push((
            offset_sample(curve, distance, normal, a, ParameterSide::Right)?,
            offset_sample(curve, distance, normal, b, ParameterSide::Left)?,
        ));
    }
    for pair in boundaries.windows(2) {
        if pair[0].1.point.distance_to(pair[1].0.point)? > tolerance.absolute() {
            return Err(GeometryError::Degenerate {
                context: "NURBS offset source kink",
            });
        }
    }
    let closed = curve.is_closed()?;
    if closed
        && boundaries
            .last()
            .unwrap()
            .1
            .point
            .distance_to(boundaries[0].0.point)?
            > tolerance.absolute()
    {
        return Err(GeometryError::Degenerate {
            context: "NURBS offset source seam kink",
        });
    }
    let mut pending = boundaries.into_iter().rev().collect::<Vec<_>>();
    let mut accepted = Vec::new();
    let target = tolerance.absolute() * 0.25;
    while let Some((start, end)) = pending.pop() {
        let controls = cubic_controls(start, end)?;
        let mut largest = 0.0_f64;
        for check in 1..=CHECKS_PER_SPAN {
            let fraction = check as Real / (CHECKS_PER_SPAN + 1) as Real;
            let parameter = start
                .parameter
                .mul_add(1.0 - fraction, end.parameter * fraction);
            let expected =
                offset_sample(curve, distance, normal, parameter, ParameterSide::Right)?.point;
            largest = largest.max(cubic_point(controls, fraction)?.distance_to(expected)?);
        }
        if largest <= target {
            accepted.push((start, end, controls));
            continue;
        }
        if accepted.len() + pending.len() >= MAX_OFFSET_SPANS {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        let middle_parameter = start.parameter.midpoint(end.parameter);
        if middle_parameter <= start.parameter || middle_parameter >= end.parameter {
            return Err(GeometryError::NurbsOffsetFitLimit);
        }
        let middle = offset_sample(
            curve,
            distance,
            normal,
            middle_parameter,
            ParameterSide::Right,
        )?;
        pending.push((middle, end));
        pending.push((start, middle));
    }
    let mut points = Vec::with_capacity(3 * accepted.len() + 1);
    let mut knots = vec![accepted[0].0.parameter; 4];
    points.push(accepted[0].0.point);
    for (index, (_, end, controls)) in accepted.iter().enumerate() {
        points.push(controls[1]);
        points.push(controls[2]);
        points.push(if closed && index + 1 == accepted.len() {
            points[0]
        } else {
            controls[3]
        });
        knots.extend(std::iter::repeat_n(
            end.parameter,
            if index + 1 == accepted.len() { 4 } else { 3 },
        ));
    }
    let result = NurbsCurve::try_new(3, points, knots)?;
    for (start, end, _) in &accepted {
        for check in 1..=CHECKS_PER_SPAN {
            let fraction = check as Real / (CHECKS_PER_SPAN + 1) as Real;
            let parameter = start
                .parameter
                .mul_add(1.0 - fraction, end.parameter * fraction);
            let expected =
                offset_sample(curve, distance, normal, parameter, ParameterSide::Right)?.point;
            if result.evaluate(parameter)?.distance_to(expected)? > tolerance.absolute() {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
        }
    }
    Ok(result)
}

fn signed_distance(
    curve: &NurbsCurve,
    point: Point3,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let normal = offset_plane(curve, fallback, tolerance)?;
    let parameter = curve.closest_parameter(point, tolerance)?;
    let (nearest, tangent) = curve.evaluate_with_derivative(parameter)?;
    let left = normal.as_vector().cross(tangent)?.normalized_nonzero()?;
    nearest.vector_to(point)?.dot(left.as_vector())
}

pub(super) fn nurbs_offset_side(
    curve: &NurbsCurve,
    point: Point3,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let signed = signed_distance(curve, point, fallback, tolerance)?;
    if signed.abs() <= tolerance.absolute() {
        return Err(GeometryError::AmbiguousCurveOffsetSide);
    }
    Ok(signed.signum())
}

pub(super) fn nurbs_through_distance(
    curve: &NurbsCurve,
    point: Point3,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    signed_distance(curve, point, fallback, tolerance)
}
