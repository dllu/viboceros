//! Source-space acceptance and ordering for external curve intersection producers.
use super::*;
use crate::ParameterSide;

/// Proposed paired parameters. An end pair represents an overlap range;
/// absent end parameters represent a point contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveCurveIntersectionParameters {
    pub start: [Real; 2],
    pub end: Option<[Real; 2]>,
}

pub(super) fn distance_tolerance(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: Tolerance,
) -> (Real, Real) {
    let scale = first
        .control_points
        .iter()
        .chain(&second.control_points)
        .flat_map(|control| control.point.to_array())
        .fold(1_f64, |scale, coordinate| scale.max(coordinate.abs()));
    (
        scale,
        tolerance.absolute().max(tolerance.relative() * scale),
    )
}

impl NurbsCurve {
    /// Use an external point/range producer, retaining source-space acceptance,
    /// deduplication and event ordering. `None` selects the portable search.
    ///
    /// The producer receives the effective model-space distance tolerance.
    /// Returned point contacts are evaluated on both original curves. Overlap
    /// interiors are independently checked in both directions. These sampled
    /// checks do not certify event completeness or continuous coincidence.
    pub fn intersection_events_with_curve_with_operation<E: From<GeometryError>>(
        &self,
        other: &Self,
        tolerance: Tolerance,
        operation: &mut impl FnMut(
            &Self,
            &Self,
            Real,
        ) -> Result<Option<Vec<CurveCurveIntersectionParameters>>, E>,
    ) -> Result<Vec<CurveCurveIntersectionEvent>, E> {
        self.tight_bounds(tolerance)?;
        other.tight_bounds(tolerance)?;
        let (_, limit) = distance_tolerance(self, other, tolerance);
        let Some(parameters) = operation(self, other, limit)? else {
            return Ok(self.intersection_events_with_curve(other, tolerance)?);
        };
        if parameters.len() > 500_000 {
            return Err(GeometryError::CurveIntersectionDidNotConverge.into());
        }
        let point = |parameters: [Real; 2]| -> Result<CurveCurveIntersection, GeometryError> {
            let a = self.evaluate(parameters[0])?;
            let b = other.evaluate(parameters[1])?;
            if a.distance_to(b)? > limit {
                return Err(GeometryError::CurveIntersectionDidNotConverge);
            }
            Ok(CurveCurveIntersection {
                first_parameter: parameters[0],
                second_parameter: parameters[1],
                point: a.midpoint(b)?,
            })
        };
        let mut intersections = Vec::new();
        let mut overlaps = Vec::new();
        let mut checks = 0;
        for proposed in parameters {
            let mut start = point(proposed.start)?;
            if let Some(end) = proposed.end {
                let mut end = point(end)?;
                if start.first_parameter > end.first_parameter {
                    std::mem::swap(&mut start, &mut end);
                }
                if start.first_parameter == end.first_parameter
                    && start.second_parameter == end.second_parameter
                {
                    push_unique_curve_intersection(&mut intersections, start, limit);
                    continue;
                }
                if start.first_parameter == end.first_parameter
                    || start.second_parameter == end.second_parameter
                {
                    return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                }
                let overlap = CurveCurveOverlap { start, end };
                let a = self.try_trimmed(overlap.first_interval())?;
                let b = other.try_trimmed(overlap.second_interval())?;
                for (source, target) in [(&a, &b), (&b, &a)] {
                    for (lo, hi) in source.spans() {
                        for station in 0..=16 {
                            checks += 1;
                            if checks > 500_000 {
                                return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                            }
                            let t = lo
                                .mul_add(1. - station as Real / 16., hi * (station as Real / 16.));
                            let side = if station == 16 {
                                ParameterSide::Left
                            } else {
                                ParameterSide::Right
                            };
                            let p = source.evaluate_on_side(t, side)?;
                            let q = target.evaluate(target.closest_parameter(p, tolerance)?)?;
                            if p.distance_to(q)? > limit {
                                return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                            }
                        }
                    }
                }
                push_unique_curve_overlap(&mut overlaps, overlap, limit);
            } else {
                let start = polish_contact(self, other, start, tolerance, limit)?;
                push_unique_curve_intersection(&mut intersections, start, limit);
            }
        }
        Ok(finalize(intersections, overlaps, limit))
    }
}

fn polish_contact(
    first: &NurbsCurve,
    second: &NurbsCurve,
    mut current: CurveCurveIntersection,
    tolerance: Tolerance,
    limit: Real,
) -> Result<CurveCurveIntersection, GeometryError> {
    // Refine only a contact-uncertainty neighborhood inside the proposed span pair.
    // A global closest query could move to another visit of a looping curve.
    let Some((lo, hi)) = first
        .spans()
        .find(|(lo, hi)| *lo <= current.first_parameter && current.first_parameter <= *hi)
    else {
        return Ok(current);
    };
    let Some((u, v)) = second
        .spans()
        .find(|(lo, hi)| *lo <= current.second_parameter && current.second_parameter <= *hi)
    else {
        return Ok(current);
    };
    let neighborhood = |curve: &NurbsCurve| -> Result<Real, GeometryError> {
        let origin = curve.control_points[0].point;
        let mut scale = 0_f64;
        for control in &curve.control_points {
            scale = scale.max(origin.distance_to(control.point)?);
        }
        let relative = if scale > 0. { limit / scale } else { 0. };
        Ok((relative.sqrt() * 8.)
            .max(Real::EPSILON.sqrt() * 64.)
            .min(0.01))
    };
    let first_step = (hi - lo) * neighborhood(first)?;
    let second_step = (v - u) * neighborhood(second)?;
    let lo = lo.max(current.first_parameter - first_step);
    let hi = hi.min(current.first_parameter + first_step);
    let u = u.max(current.second_parameter - second_step);
    let v = v.min(current.second_parameter + second_step);
    if lo >= hi || u >= v {
        return Ok(current);
    }
    if let Ok(Some(refined)) = refine_curve_curve_intersection(
        first,
        second,
        current.first_parameter,
        current.second_parameter,
        [lo, hi],
        [u, v],
        limit,
    ) {
        current = refined;
    }
    let a = first
        .derivative_at(current.first_parameter)?
        .normalized_nonzero();
    let b = second
        .derivative_at(current.second_parameter)?
        .normalized_nonzero();
    let (Ok(a), Ok(b)) = (a, b) else {
        return Ok(current);
    };
    if a.as_vector().cross(b.as_vector())?.length()? > Real::EPSILON.sqrt() * 64. {
        return Ok(current);
    }
    let search = second.try_trimmed(u..=v)?;
    if let Some((refined, distance)) =
        refine_curve_curve_tangency(first, second, &search, current, [lo, hi], tolerance)?
        && distance <= limit
    {
        let a = first
            .derivative_at(refined.first_parameter)?
            .normalized_nonzero();
        let b = second
            .derivative_at(refined.second_parameter)?
            .normalized_nonzero();
        if let (Ok(a), Ok(b)) = (a, b)
            && a.as_vector().cross(b.as_vector())?.length()?
                <= tolerance.angular().max(Real::EPSILON * 512.)
        {
            return Ok(refined);
        }
    }
    Ok(current)
}

pub(super) fn finalize(
    mut intersections: Vec<CurveCurveIntersection>,
    mut overlaps: Vec<CurveCurveOverlap>,
    limit: Real,
) -> Vec<CurveCurveIntersectionEvent> {
    overlaps.sort_by(compare_curve_overlaps);
    let overlaps = merge_adjacent_curve_overlaps(overlaps, limit);
    intersections.retain(|intersection| {
        !overlaps
            .iter()
            .any(|overlap| curve_overlap_contains_intersection(*overlap, *intersection))
    });
    intersections.sort_by(compare_curve_intersections);
    let mut events = intersections
        .into_iter()
        .map(CurveCurveIntersectionEvent::Point)
        .chain(
            overlaps
                .into_iter()
                .map(CurveCurveIntersectionEvent::Overlap),
        )
        .collect::<Vec<_>>();
    events.sort_by(|a, b| {
        curve_intersection_event_parameter(*a).total_cmp(&curve_intersection_event_parameter(*b))
    });
    events
}

#[cfg(test)]
mod tests;
