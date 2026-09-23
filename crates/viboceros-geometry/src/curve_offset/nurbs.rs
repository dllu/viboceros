//! Tolerance-checked cubic approximations of smooth planar NURBS offsets.

use crate::{
    Brep, GeometryError, NurbsCurve, ParameterSide, Point3, Real, Tolerance, UnitVector3, Vector3,
};

const MAX_OFFSET_SPANS: usize = 8_192;
const CHECKS_PER_SPAN: usize = 15;
const MAX_REGION_PIECES: usize = 8_192;

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

/// A positive-weight Bezier curve whose controls are ordered along its endpoint
/// chord cannot revisit a point. Split until this holds, then test distinct
/// pieces for every contact other than their common boundary endpoint.
fn simple_region_pieces(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    let mut pending = curve
        .try_bezier_spans()?
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let mut pieces = Vec::new();
    while let Some(piece) = pending.pop() {
        if pieces.len() + pending.len() + 1 > MAX_REGION_PIECES {
            return Err(GeometryError::SelfIntersectingOffsetRegion);
        }
        let controls = piece.control_points();
        if controls.iter().any(|control| {
            control.weight().is_sign_positive() != controls[0].weight().is_sign_positive()
        }) {
            return Err(GeometryError::SelfIntersectingOffsetRegion);
        }
        let chord = controls[0]
            .point()
            .vector_to(controls.last().unwrap().point())?;
        let chord_length = chord.length()?;
        let monotone = chord_length > tolerance.absolute()
            && controls.windows(2).all(|pair| {
                let delta = pair[0].point().vector_to(pair[1].point());
                delta
                    .and_then(|vector| vector.dot(chord))
                    .is_ok_and(|value| value >= 0.0)
            });
        if monotone {
            pieces.push(piece);
            continue;
        }
        let domain = piece.domain();
        let midpoint = domain.start().midpoint(*domain.end());
        if midpoint <= *domain.start() || midpoint >= *domain.end() {
            return Err(GeometryError::SelfIntersectingOffsetRegion);
        }
        let (first, second) = piece.try_split(midpoint)?;
        pending.push(second);
        pending.push(first);
    }
    if pieces.len() < 2 {
        // A closed injective piece cannot enclose a nonzero region.
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    let mut endpoints = Vec::with_capacity(pieces.len());
    for piece in &pieces {
        let domain = piece.domain();
        endpoints.push((
            piece.evaluate(*domain.start())?,
            piece.evaluate(*domain.end())?,
        ));
    }
    for i in 0..pieces.len() {
        if endpoints[i]
            .1
            .distance_to(endpoints[(i + 1) % pieces.len()].0)?
            > tolerance.absolute()
        {
            return Err(GeometryError::SelfIntersectingOffsetRegion);
        }
    }
    let boxes = pieces
        .iter()
        .map(NurbsCurve::control_point_bounds)
        .collect::<Vec<_>>();
    let mut sweep = (0..pieces.len()).collect::<Vec<_>>();
    sweep.sort_by(|&a, &b| boxes[a].min().x().total_cmp(&boxes[b].min().x()));
    for (position, &index) in sweep.iter().enumerate() {
        for &other in &sweep[position + 1..] {
            if boxes[other].min().x() > boxes[index].max().x() + tolerance.absolute() {
                break;
            }
            let (i, j) = (index.min(other), index.max(other));
            let adjacent = j == i + 1 || i == 0 && j + 1 == pieces.len();
            if (1..3).any(|axis| {
                boxes[i].max().to_array()[axis] + tolerance.absolute()
                    < boxes[j].min().to_array()[axis]
                    || boxes[j].max().to_array()[axis] + tolerance.absolute()
                        < boxes[i].min().to_array()[axis]
            }) {
                continue;
            }
            let events = pieces[i].intersection_events_with_curve(&pieces[j], tolerance)?;
            if !adjacent && !events.is_empty() {
                return Err(GeometryError::SelfIntersectingOffsetRegion);
            }
            if adjacent {
                let shared = if j == i + 1 {
                    endpoints[i].1
                } else {
                    endpoints[i].0
                };
                if events.len() != 1
                    || !matches!(events[0], crate::CurveCurveIntersectionEvent::Point(event) if event.point().distance_to(shared)? <= tolerance.absolute())
                {
                    return Err(GeometryError::SelfIntersectingOffsetRegion);
                }
            }
        }
    }
    Ok(pieces)
}

pub(super) fn nurbs_region_inward_sign(
    curve: &NurbsCurve,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    let normal = offset_plane(curve, fallback, tolerance)?;
    let _pieces = simple_region_pieces(curve, tolerance)?;
    // The planar face validates the trim and rational denominator, including
    // cases where the Bezier control hull does not bound the attained curve.
    let face = Brep::try_planar_face(curve, tolerance)?;
    let face = &face.faces()[0];
    let surface = face.surface();
    let u = surface.domain_u();
    let v = surface.domain_v();
    let (_, du, dv) = surface
        .evaluate_with_derivatives(u.start().midpoint(*u.end()), v.start().midpoint(*v.end()))?;
    let frame_sign = du.cross(dv)?.dot(normal.as_vector())?.signum();
    if frame_sign == 0.0 {
        return Err(GeometryError::DegenerateOffsetRegion);
    }
    let source_sign = if face.loops()[0].trims()[0].is_reversed_3d() {
        -1.0
    } else {
        1.0
    };
    Ok(frame_sign * source_sign)
}

pub(super) fn nurbs_region_contains(
    curve: &NurbsCurve,
    point: Point3,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let normal = offset_plane(curve, fallback, tolerance)?;
    let origin = curve.evaluate(*curve.domain().start())?;
    if origin.vector_to(point)?.dot(normal.as_vector())?.abs() > tolerance.absolute() {
        return Ok(false);
    }
    let nearest = curve.evaluate(curve.closest_parameter(point, tolerance)?)?;
    if nearest.distance_to(point)? <= tolerance.absolute() {
        return Err(GeometryError::AmbiguousCurveOffsetSide);
    }
    simple_region_pieces(curve, tolerance)?;
    let face = Brep::try_planar_face(curve, tolerance)?;
    let (index, u, v) = face.closest_underlying_face_parameters(point, tolerance)?;
    if face.faces()[index]
        .surface()
        .evaluate(u, v)?
        .distance_to(point)?
        > tolerance.absolute()
    {
        return Ok(false);
    }
    face.faces()[index].contains_parameters(u, v, tolerance)
}
