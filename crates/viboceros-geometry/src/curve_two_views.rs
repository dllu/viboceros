//! Two-view reconstruction as the intersection of bounded ruled projectors.
use crate::{
    GeometryError, NurbsCurve, NurbsSurface, Real, SurfaceSurfaceIntersectionEvent, Tolerance,
    Vector3,
};
mod aligned;
#[cfg(test)]
mod tests;

pub const MAX_TWO_VIEW_CONTROLS: usize = 4096;
pub const MAX_TWO_VIEW_OUTPUTS: usize = 4096;
fn limit(resource: &'static str, maximum: usize) -> GeometryError {
    GeometryError::CurveTwoViewsLimit { resource, maximum }
}

/// Reconstruct the common spatial image of two profile projections. Parallel
/// directions are rejected before construction. The supplied intersector owns
/// general curved intersection solving; its usual certification still applies.
/// A shared monotone chart permits exact control-wise reconstruction first.
pub fn curves_from_two_views_with_operation<E: From<GeometryError>>(
    curves: [&NurbsCurve; 2],
    directions: [Vector3; 2],
    tolerance: Tolerance,
    intersect: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Tolerance,
    ) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, E>,
) -> Result<Vec<NurbsCurve>, E> {
    let directions = validate(curves, directions, tolerance)?;
    if let Some(curve) = aligned::reconstruct(curves, directions)? {
        return Ok(vec![curve]);
    }
    let [a, b] = projectors(curves, directions, tolerance)?;
    let events = intersect(&a, &b, tolerance)?;
    if events.len() > MAX_TWO_VIEW_OUTPUTS {
        return Err(limit("intersection outputs", MAX_TWO_VIEW_OUTPUTS).into());
    }
    let mut controls = 0usize;
    let mut result = Vec::new();
    for event in events {
        if let SurfaceSurfaceIntersectionEvent::Curve(curve) = event {
            controls = controls.saturating_add(curve.control_points().len());
            if controls > 100_000 {
                return Err(limit("output controls", 100_000).into());
            }
            result.push(curve);
        }
    }
    Ok(result)
}

pub fn curves_from_two_views(
    curves: [&NurbsCurve; 2],
    directions: [Vector3; 2],
    tolerance: Tolerance,
) -> Result<Vec<NurbsCurve>, GeometryError> {
    curves_from_two_views_with_operation(
        curves,
        directions,
        tolerance,
        &mut crate::surface_surface_intersection_events,
    )
}

fn validate(
    curves: [&NurbsCurve; 2],
    directions: [Vector3; 2],
    tolerance: Tolerance,
) -> Result<[Vector3; 2], GeometryError> {
    if curves
        .iter()
        .map(|c| c.control_points().len())
        .fold(0usize, usize::saturating_add)
        > MAX_TWO_VIEW_CONTROLS
    {
        return Err(limit("input controls", MAX_TWO_VIEW_CONTROLS));
    }
    for curve in curves {
        let positive = curve.control_points()[0].weight() > 0.;
        if curve
            .control_points()
            .iter()
            .any(|p| (p.weight() > 0.) != positive)
        {
            return Err(GeometryError::InvalidControlNet {
                context: "two-view reconstruction needs common-sign curve weights",
            });
        }
    }
    let [a, b] = directions.map(|d| d.normalized_nonzero().map(|d| d.as_vector()));
    let [a, b] = [a?, b?];
    let sine = a.cross(b)?.length()?;
    if sine <= tolerance.angular().min(std::f64::consts::FRAC_PI_2).sin() {
        return Err(GeometryError::ParallelCurveViewDirections);
    }
    Ok([a, b])
}

// Solve C(u)+alpha*a = D(v)+beta*b. Each coefficient is a linear
// functional of D-C. Common-sign rational hulls bound every possible contact
// using O(n+m) extrema, without an arbitrary extrusion length or pair search.
fn projectors(
    curves: [&NurbsCurve; 2],
    [a, b]: [Vector3; 2],
    tolerance: Tolerance,
) -> Result<[NurbsSurface; 2], GeometryError> {
    let n = a.cross(b)?;
    let sine = n.length()?;
    let dual = [
        b.cross(n)?.scaled(1. / (sine * sine))?,
        a.cross(n)?.scaled(1. / (sine * sine))?,
    ];
    let origin = curves[0].control_points()[0].point();
    let mut ranges = [[0.; 2]; 2];
    let mut scale = 0_f64;
    for (i, functional) in dual.into_iter().enumerate() {
        let mut bounds = [[Real::INFINITY, Real::NEG_INFINITY]; 2];
        for (j, curve) in curves.iter().enumerate() {
            for control in curve.control_points() {
                let value = functional.dot_point_difference(control.point(), origin);
                crate::require_finite([value], "two-view projector extent")?;
                bounds[j][0] = bounds[j][0].min(value);
                bounds[j][1] = bounds[j][1].max(value);
                scale = scale.max(value.abs());
            }
        }
        ranges[i] = [bounds[1][0] - bounds[0][1], bounds[1][1] - bounds[0][0]];
    }
    let margin = tolerance.absolute() * 16. / sine + scale * Real::EPSILON * 128.;
    let offsets = ranges.map(|r| [r[0] - margin, r[1] + margin]);
    Ok([
        NurbsSurface::try_extruded_curve(
            curves[0],
            a.scaled(offsets[0][0])?,
            a.scaled(offsets[0][1])?,
        )?,
        NurbsSurface::try_extruded_curve(
            curves[1],
            b.scaled(offsets[1][0])?,
            b.scaled(offsets[1][1])?,
        )?,
    ])
}
