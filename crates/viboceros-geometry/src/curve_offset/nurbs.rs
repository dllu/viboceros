//! Tolerance-checked cubic approximations of smooth planar NURBS offsets.

use crate::{
    Brep, Circle3, CircularArc3, Curve3, CurveCurveIntersectionEvent, CurveSegment3, GeometryError,
    LineSegment, NurbsCurve, ParameterSide, Point3, PolyCurve3, Polyline3, Real, Tolerance,
    UnitVector3, Vector3, WeightedPoint3, nurbs::curve_points_coincident,
};

const MAX_OFFSET_SPANS: usize = 8_192;
const CHECKS_PER_SPAN: usize = 15;
const MAX_REGION_PIECES: usize = 8_192;

mod closed_offset;
mod sharp_offset;
use closed_offset::offset_nurbs_closed_gaps;
pub(super) use sharp_offset::offset_nurbs_sharp;

/// Degree-one, uniform-weight spans are exactly affine in their native knot
/// intervals. Preserve every knot as a polyline vertex for corner handling.
pub(super) fn linear_nurbs_proxy(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<Polyline3>, GeometryError> {
    linear_nurbs_polyline(curve, tolerance, 2)
}

pub(super) fn linear_nurbs_leaf_proxy(
    curve: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<Polyline3>, GeometryError> {
    linear_nurbs_polyline(curve, tolerance, 1)
}

fn linear_nurbs_polyline(
    curve: &NurbsCurve,
    tolerance: Tolerance,
    minimum_spans: usize,
) -> Result<Option<Polyline3>, GeometryError> {
    if curve.degree() != 1
        || curve
            .control_points()
            .iter()
            .any(|control| control.weight() != curve.control_points()[0].weight())
    {
        return Ok(None);
    }
    let spans = curve.spans().collect::<Vec<_>>();
    if spans.len() < minimum_spans {
        return Ok(None);
    }
    let mut vertices = Vec::with_capacity(spans.len() + 1);
    let mut parameters = Vec::with_capacity(spans.len() + 1);
    for (index, &(a, b)) in spans.iter().enumerate() {
        let start = curve.evaluate_on_side(a, ParameterSide::Right)?;
        if index == 0 {
            vertices.push(start);
            parameters.push(a);
        } else if !curve_points_coincident(*vertices.last().unwrap(), start)
            || vertices.last().unwrap().distance_to(start)? > tolerance.absolute()
        {
            return Err(GeometryError::Degenerate {
                context: "discontinuous linear NURBS offset source",
            });
        }
        vertices.push(curve.evaluate_on_side(b, ParameterSide::Left)?);
        parameters.push(b);
    }
    Ok(Some(Polyline3::try_with_parameters(
        vertices, parameters, tolerance,
    )?))
}

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
    offset_nurbs_with_closure(curve, distance, fallback, tolerance, curve.is_closed()?)
}

fn offset_nurbs_with_closure(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
    closed: bool,
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

/// Keep convex gaps open and trim concave kinks at the nearest transverse
/// intersection of the neighboring offset loci.
pub(super) fn offset_nurbs_open_gaps(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Vec<Curve3>, GeometryError> {
    let normal = offset_plane(curve, fallback, tolerance)?;
    let domain = curve.domain();
    let spans = curve.spans().collect::<Vec<_>>();
    let mut cuts = Vec::new();
    for pair in spans.windows(2) {
        let parameter = pair[0].1;
        let (left_point, left_velocity) =
            curve.evaluate_with_derivative_on_side(parameter, ParameterSide::Left)?;
        let (right_point, right_velocity) =
            curve.evaluate_with_derivative_on_side(parameter, ParameterSide::Right)?;
        if let Some(convex) = classify_offset_kink(
            left_point,
            left_velocity,
            right_point,
            right_velocity,
            normal,
            distance,
            tolerance,
        )? {
            cuts.push((parameter, convex));
        }
    }
    let closed = curve.is_closed()?;
    let seam = if closed {
        let (left_point, left_velocity) =
            curve.evaluate_with_derivative_on_side(*domain.end(), ParameterSide::Left)?;
        let (right_point, right_velocity) =
            curve.evaluate_with_derivative_on_side(*domain.start(), ParameterSide::Right)?;
        classify_offset_kink(
            left_point,
            left_velocity,
            right_point,
            right_velocity,
            normal,
            distance,
            tolerance,
        )?
    } else {
        None
    };
    if cuts.is_empty() && seam.is_none() {
        return Ok(vec![Curve3::NurbsCurve(offset_nurbs(
            curve, distance, fallback, tolerance,
        )?)]);
    }
    if closed {
        return offset_nurbs_closed_gaps(curve, distance, fallback, normal, tolerance, &cuts, seam);
    }
    debug_assert!(
        cuts.iter()
            .all(|&(parameter, _)| parameter > *domain.start() && parameter < *domain.end())
    );
    let parameters = cuts
        .iter()
        .map(|&(parameter, _)| parameter)
        .collect::<Vec<_>>();
    let sources = curve.try_split_at_parameters(&parameters)?;
    let offsets = sources
        .iter()
        .map(|piece| offset_nurbs(piece, distance, fallback, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut trims = offsets
        .iter()
        .map(|piece| [*piece.domain().start(), *piece.domain().end()])
        .collect::<Vec<_>>();
    for (index, &(_, convex)) in cuts.iter().enumerate() {
        if convex {
            continue;
        }
        let corner = sources[index].evaluate(*sources[index].domain().end())?;
        let mut best = None;
        for event in
            offsets[index].intersection_events_with_curve(&offsets[index + 1], tolerance)?
        {
            let CurveCurveIntersectionEvent::Point(hit) = event else {
                return Err(GeometryError::Degenerate {
                    context: "overlapping NURBS offset corner",
                });
            };
            let first_parameter = hit.first_parameter();
            let second_parameter = hit.second_parameter();
            if first_parameter <= trims[index][0] || second_parameter >= trims[index + 1][1] {
                continue;
            }
            let first_tangent = offsets[index]
                .derivative_at(first_parameter)?
                .normalized_nonzero()?;
            let second_tangent = offsets[index + 1]
                .derivative_at(second_parameter)?
                .normalized_nonzero()?;
            let crossing = first_tangent
                .as_vector()
                .cross(second_tangent.as_vector())?
                .dot(normal.as_vector())?
                .abs();
            if crossing <= tolerance.angular() {
                continue;
            }
            let distance_to_corner = hit.point().distance_to(corner)?;
            if best.is_none_or(|(best_distance, _, _)| distance_to_corner < best_distance) {
                best = Some((distance_to_corner, first_parameter, second_parameter));
            }
        }
        let Some((_, first_parameter, second_parameter)) = best else {
            return Err(GeometryError::Degenerate {
                context: "concave NURBS offset corner has no transverse trim",
            });
        };
        trims[index][1] = first_parameter;
        trims[index + 1][0] = second_parameter;
    }
    let mut results = Vec::new();
    let mut group_start = 0;
    for (index, cut) in cuts
        .iter()
        .map(Some)
        .chain(std::iter::once(None))
        .enumerate()
    {
        if cut.is_some_and(|cut| !cut.1) {
            continue;
        }
        let mut segments = Vec::with_capacity(index + 1 - group_start);
        let mut breaks = vec![*sources[group_start].domain().start()];
        for part in group_start..=index {
            if trims[part][0] >= trims[part][1] {
                return Err(GeometryError::Degenerate {
                    context: "concave NURBS offset consumes a source segment",
                });
            }
            segments.push(offsets[part].try_trimmed(trims[part][0]..=trims[part][1])?);
            breaks.push(*sources[part].domain().end());
        }
        if segments.len() == 1 {
            validate_trimmed_offset(
                &sources[group_start],
                &segments[0],
                distance,
                normal,
                tolerance,
            )?;
            results.push(Curve3::NurbsCurve(segments.remove(0)));
        } else {
            for part in 0..segments.len() - 1 {
                let (left, right) = segments.split_at_mut(part + 1);
                align_trimmed_ends(&mut left[part], &mut right[0], tolerance)?;
            }
            for (part, segment) in segments.iter().enumerate() {
                validate_trimmed_offset(
                    &sources[group_start + part],
                    segment,
                    distance,
                    normal,
                    tolerance,
                )?;
            }
            results.push(Curve3::PolyCurve(PolyCurve3::try_with_segment_domains(
                segments
                    .into_iter()
                    .map(CurveSegment3::NurbsCurve)
                    .collect::<Vec<_>>(),
                breaks,
            )?));
        }
        group_start = index + 1;
    }
    Ok(results)
}

/// Join the certified smooth pieces with straight chords across convex gaps.
/// The `None` result already contains every concave trim, in source order.
pub(super) fn offset_nurbs_chamfer(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    offset_nurbs_connected(curve, distance, fallback, tolerance, false)
}

pub(super) fn offset_nurbs_round(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
) -> Result<Curve3, GeometryError> {
    offset_nurbs_connected(curve, distance, fallback, tolerance, true)
}

fn offset_nurbs_connected(
    curve: &NurbsCurve,
    distance: Real,
    fallback: UnitVector3,
    tolerance: Tolerance,
    round: bool,
) -> Result<Curve3, GeometryError> {
    let mut pieces = offset_nurbs_open_gaps(curve, distance, fallback, tolerance)?;
    let closed = curve.is_closed()?;
    if pieces.len() == 1 && (!closed || pieces[0].as_ref().is_closed()?) {
        return Ok(pieces.remove(0));
    }
    let starts = pieces
        .iter()
        .map(|piece| piece.as_ref().start_point())
        .collect::<Result<Vec<_>, _>>()?;
    let ends = pieces
        .iter()
        .map(|piece| piece.as_ref().end_point())
        .collect::<Result<Vec<_>, _>>()?;
    let start = if closed {
        match &pieces[0] {
            Curve3::NurbsCurve(part) => *part.domain().start(),
            Curve3::PolyCurve(part) => *part.domain().start(),
            _ => unreachable!("NURBS offset pieces are NURBS or polycurves"),
        }
    } else {
        *curve.domain().start()
    };
    let span = *curve.domain().end() - *curve.domain().start();
    let end = start + span;
    crate::require_finite([end], "joined NURBS offset parameter")?;
    let normal = if round {
        Some(offset_plane(curve, fallback, tolerance)?)
    } else {
        None
    };
    let mut segments = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        segments.extend(piece.to_polycurve()?.segments().iter().cloned());
        if index + 1 < pieces.len() || closed {
            let next = (index + 1) % pieces.len();
            if let Some(normal) = normal {
                let domain = curve.domain();
                let parameter = match piece {
                    Curve3::NurbsCurve(part) => *part.domain().end(),
                    Curve3::PolyCurve(part) => *part.domain().end(),
                    _ => unreachable!("NURBS offset pieces are NURBS or polycurves"),
                };
                let source_parameter = if parameter > *domain.end() {
                    parameter - span
                } else {
                    parameter
                };
                let center = curve.evaluate(source_parameter)?;
                let first = center.vector_to(ends[index])?.normalized_nonzero()?;
                let second = center.vector_to(starts[next])?.normalized_nonzero()?;
                let sine = first
                    .as_vector()
                    .cross(second.as_vector())?
                    .dot(normal.as_vector())?;
                let cosine = first.as_vector().dot(second.as_vector())?;
                let sweep = sine.abs().atan2(cosine);
                let arc_normal = if sine < 0.0 {
                    normal.opposite()
                } else {
                    normal
                };
                let circle =
                    Circle3::try_from_center_point(center, ends[index], arc_normal, tolerance)?;
                if (center.distance_to(starts[next])? - circle.radius()).abs()
                    > tolerance.absolute()
                {
                    return Err(GeometryError::NurbsOffsetFitLimit);
                }
                segments.push(CurveSegment3::Arc(CircularArc3::try_from_circle_sweep(
                    circle, sweep,
                )?));
            } else {
                segments.push(CurveSegment3::Line(LineSegment::try_new(
                    ends[index],
                    starts[next],
                    tolerance,
                )?));
            }
        }
    }
    Ok(Curve3::PolyCurve(
        PolyCurve3::try_new(segments)?.try_reparameterized(start..=end)?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn classify_offset_kink(
    left_point: Point3,
    left_velocity: Vector3,
    right_point: Point3,
    right_velocity: Vector3,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Option<bool>, GeometryError> {
    if left_point.distance_to(right_point)? > tolerance.absolute() {
        return Err(GeometryError::Degenerate {
            context: "discontinuous NURBS offset source",
        });
    }
    let left = normal
        .as_vector()
        .cross(left_velocity)?
        .normalized_nonzero()?;
    let right = normal
        .as_vector()
        .cross(right_velocity)?
        .normalized_nonzero()?;
    let a = left.as_vector().to_array();
    let b = right.as_vector().to_array();
    let separation = (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2]);
    if separation * distance.abs() <= tolerance.absolute() {
        return Ok(None);
    }
    let turn = left_velocity
        .normalized_nonzero()?
        .as_vector()
        .cross(right_velocity.normalized_nonzero()?.as_vector())?
        .dot(normal.as_vector())?;
    if turn.abs() <= tolerance.angular() {
        return Err(GeometryError::Degenerate {
            context: "NURBS offset backtracking kink",
        });
    }
    Ok(Some(turn * distance < 0.0))
}

fn validate_trimmed_offset(
    source: &NurbsCurve,
    result: &NurbsCurve,
    distance: Real,
    normal: UnitVector3,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    for (a, b) in result.spans() {
        for check in 0..=CHECKS_PER_SPAN + 1 {
            let fraction = check as Real / (CHECKS_PER_SPAN + 1) as Real;
            let parameter = a.mul_add(1.0 - fraction, b * fraction);
            let side = if check == CHECKS_PER_SPAN + 1 {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            let expected = offset_sample(source, distance, normal, parameter, side)?.point;
            if result
                .evaluate_on_side(parameter, side)?
                .distance_to(expected)?
                > tolerance.absolute()
            {
                return Err(GeometryError::NurbsOffsetFitLimit);
            }
        }
    }
    Ok(())
}

fn align_trimmed_ends(
    first: &mut NurbsCurve,
    second: &mut NurbsCurve,
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let a = first.evaluate(*first.domain().end())?;
    let b = second.evaluate(*second.domain().start())?;
    if a.distance_to(b)? > tolerance.absolute() * 0.1 {
        return Err(GeometryError::NurbsOffsetFitLimit);
    }
    if curve_points_coincident(a, b) {
        return Ok(());
    }
    let shared = a.midpoint(b)?;
    let mut first_controls = first.control_points().to_vec();
    let mut second_controls = second.control_points().to_vec();
    let last = first_controls.len() - 1;
    first_controls[last] = WeightedPoint3::try_new(shared, first_controls[last].weight())?;
    second_controls[0] = WeightedPoint3::try_new(shared, second_controls[0].weight())?;
    *first = NurbsCurve::try_new_rational(first.degree(), first_controls, first.knots().to_vec())?;
    *second =
        NurbsCurve::try_new_rational(second.degree(), second_controls, second.knots().to_vec())?;
    Ok(())
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
                if pieces.len() == 2 {
                    let [
                        crate::CurveCurveIntersectionEvent::Point(first),
                        crate::CurveCurveIntersectionEvent::Point(second),
                    ] = events.as_slice()
                    else {
                        return Err(GeometryError::SelfIntersectingOffsetRegion);
                    };
                    let seam = endpoints[0].0;
                    let middle = endpoints[0].1;
                    let direct = first.point().distance_to(seam)? <= tolerance.absolute()
                        && second.point().distance_to(middle)? <= tolerance.absolute();
                    let reverse = first.point().distance_to(middle)? <= tolerance.absolute()
                        && second.point().distance_to(seam)? <= tolerance.absolute();
                    if !direct && !reverse {
                        return Err(GeometryError::SelfIntersectingOffsetRegion);
                    }
                } else {
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
