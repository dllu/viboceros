//! Source-space validation and event policy for external curve/surface producers.
use super::*;
use crate::ParameterSide;

/// Proposed curve/U/V parameters, with optional overlap-end parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSurfaceIntersectionParameters {
    pub start: [Real; 3],
    pub end: Option<[Real; 3]>,
}

/// Use an external point/range producer, retaining original-geometry acceptance.
/// `None` selects the portable search. The producer receives the effective
/// model-space distance tolerance. Overlap interiors are sampled per source span;
/// these checks do not prove continuous coincidence or event completeness.
pub fn curve_surface_intersection_events_with_operation<E: From<GeometryError>>(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
    operation: &mut impl FnMut(
        &NurbsCurve,
        &NurbsSurface,
        Real,
    ) -> Result<Option<Vec<CurveSurfaceIntersectionParameters>>, E>,
) -> Result<Vec<CurveSurfaceIntersectionEvent>, E> {
    curve.tight_bounds(tolerance)?;
    let limit = curve_surface_distance_tolerance(curve, surface, tolerance);
    let Some(parameters) = operation(curve, surface, limit)? else {
        return Ok(curve_surface_intersection_events(
            curve, surface, tolerance,
        )?);
    };
    if parameters.len() > 500_000 {
        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
    }
    let point = |[t, u, v]: [Real; 3]| -> Result<CurveSurfaceIntersection, GeometryError> {
        let a = curve.evaluate(t)?;
        let b = surface.evaluate(u, v)?;
        let distance = a.distance_to(b)?;
        if distance > limit {
            return Err(GeometryError::CurveIntersectionDidNotConverge);
        }
        Ok(CurveSurfaceIntersection {
            curve_parameter: t,
            u,
            v,
            point: a.midpoint(b)?,
            distance,
        })
    };
    let mut points = Vec::new();
    let mut overlaps = Vec::new();
    let mut checks = 0;
    for proposed in parameters {
        let mut start = point(proposed.start)?;
        if let Some(end) = proposed.end {
            let mut end = point(end)?;
            if start.curve_parameter > end.curve_parameter {
                std::mem::swap(&mut start, &mut end);
            }
            if start.curve_parameter == end.curve_parameter {
                push_unique_curve_surface_intersection_with_distance(&mut points, start, limit);
                continue;
            }
            let piece = curve.try_trimmed(start.curve_parameter..=end.curve_parameter)?;
            for (lo, hi) in piece.spans() {
                for station in 0..=16 {
                    checks += 1;
                    if checks > 500_000 {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                    let t = lo.mul_add(1. - station as Real / 16., hi * (station as Real / 16.));
                    let p = piece.evaluate_on_side(
                        t,
                        if station == 16 {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                    )?;
                    let (u, v) = surface.closest_parameters(p, tolerance)?;
                    if p.distance_to(surface.evaluate(u, v)?)? > limit {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                }
            }
            overlaps.push(CurveSurfaceOverlap { start, end });
        } else {
            start = polish(curve, surface, start, tolerance, limit)?;
            push_unique_curve_surface_intersection_with_distance(&mut points, start, limit);
        }
    }
    overlaps.sort_by(|a, b| compare_curve_surface_intersections(&a.start, &b.start));
    let mut merged: Vec<CurveSurfaceOverlap> = Vec::new();
    for overlap in overlaps {
        if let Some(previous) = merged.last_mut() {
            if previous.end.curve_parameter >= overlap.start.curve_parameter {
                if previous.end.curve_parameter < overlap.end.curve_parameter {
                    previous.end = overlap.end;
                }
                continue;
            }
            if curve_surface_intersections_match(previous.end, overlap.start, limit) {
                previous.end = overlap.end;
                continue;
            }
        }
        merged.push(overlap);
    }
    Ok(finalize(points, merged))
}

pub(super) fn finalize(
    mut points: Vec<CurveSurfaceIntersection>,
    overlaps: Vec<CurveSurfaceOverlap>,
) -> Vec<CurveSurfaceIntersectionEvent> {
    points.retain(|point| {
        !overlaps.iter().any(|overlap| {
            parameter_inside_interval(
                point.curve_parameter,
                overlap.start.curve_parameter,
                overlap.end.curve_parameter,
            )
        })
    });
    points.sort_by(compare_curve_surface_intersections);
    let mut events = points
        .into_iter()
        .map(CurveSurfaceIntersectionEvent::Point)
        .chain(
            overlaps
                .into_iter()
                .map(CurveSurfaceIntersectionEvent::Overlap),
        )
        .collect::<Vec<_>>();
    events.sort_by(|a, b| {
        curve_surface_event_parameter(*a).total_cmp(&curve_surface_event_parameter(*b))
    });
    events
}

fn containing_span(mut spans: impl Iterator<Item = (Real, Real)>, t: Real) -> Option<(Real, Real)> {
    spans.find(|(a, b)| *a <= t && t <= *b)
}

fn polish(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    mut current: CurveSurfaceIntersection,
    tolerance: Tolerance,
    limit: Real,
) -> Result<CurveSurfaceIntersection, GeometryError> {
    let Some((lo, hi)) = containing_span(curve.spans(), current.curve_parameter) else {
        return Ok(current);
    };
    let Some((u0, u1)) = containing_span(surface.spans_u(), current.u) else {
        return Ok(current);
    };
    let Some((v0, v1)) = containing_span(surface.spans_v(), current.v) else {
        return Ok(current);
    };
    let origin = curve.control_points()[0].point();
    let mut extent = 0_f64;
    for p in curve
        .control_points()
        .iter()
        .chain(surface.control_points())
    {
        extent = extent.max(origin.distance_to(p.point())?);
    }
    let fraction = if extent > 0. {
        (8. * (limit / extent).sqrt())
            .max(64. * Real::EPSILON.sqrt())
            .min(0.01)
    } else {
        0.01
    };
    let window =
        |t: Real, a: Real, b: Real| [a.max(t - (b - a) * fraction), b.min(t + (b - a) * fraction)];
    let t = window(current.curve_parameter, lo, hi);
    let u = window(current.u, u0, u1);
    let v = window(current.v, v0, v1);
    if let Ok(Some(refined)) = refine_curve_surface_intersection(
        curve,
        surface,
        current.curve_parameter,
        current.u,
        current.v,
        t,
        u,
        v,
        0.,
        limit,
    ) {
        current = refined;
    }
    if let Ok((_, value)) = curve_surface_tangency_sample(
        curve,
        surface,
        current.curve_parameter,
        current.u,
        current.v,
        tolerance,
    ) && value.abs() <= 64. * Real::EPSILON.sqrt()
        && let Ok(Some((refined, angle))) =
            refine_curve_surface_tangency(curve, surface, current, value, t, tolerance)
        && refined.distance <= limit
        && angle.abs() <= tolerance.angular().max(512. * Real::EPSILON)
    {
        return Ok(refined);
    }
    Ok(current)
}

#[cfg(test)]
mod tests;
