//! Continuous original-surface acceptance for coupled intersection proposals.
use super::*;
use crate::NurbsCurve2;

/// A spatial intersection proposal with parameter curves on both sources.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceSurfaceIntersectionProposal {
    Point(Point3),
    Curve {
        spatial: NurbsCurve,
        first_uv: NurbsCurve2,
        second_uv: NurbsCurve2,
    },
}

/// Validate external surface intersection proposals against the original inputs.
/// `None` chooses the portable producer. Every curve requires continuous
/// matched-parameter correspondence proofs on both surfaces. These proofs do
/// not establish completeness or topology. Certified curve payloads are retained
/// verbatim: joining approximate endpoints would invalidate their UV witnesses.
pub fn surface_surface_intersection_events_with_operation<E: From<GeometryError>>(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
    operation: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Real,
    ) -> Result<Option<Vec<SurfaceSurfaceIntersectionProposal>>, E>,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, E> {
    let limit = surface_surface_distance_tolerance(first, second, tolerance);
    let Some(proposals) = operation(first, second, limit)? else {
        return Ok(surface_surface_intersection_events(
            first, second, tolerance,
        )?);
    };
    if proposals.len() > 500_000 {
        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
    }
    let coplanar = match (first.plane(tolerance)?, second.plane(tolerance)?) {
        (Some(a), Some(b)) => planes_are_coincident(a, b, tolerance, limit)?,
        _ => false,
    };
    let mut points = Vec::new();
    let mut curves = Vec::new();
    for proposal in proposals {
        match proposal {
            SurfaceSurfaceIntersectionProposal::Point(point) => {
                for surface in [first, second] {
                    let (u, v) = surface.closest_parameters(point, tolerance)?;
                    if surface.evaluate(u, v)?.distance_to(point)? > limit {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                }
                // A lone coplanar corner is excluded by the measured command policy.
                if !coplanar {
                    push_unique_brep_point(&mut points, point, limit);
                }
            }
            SurfaceSurfaceIntersectionProposal::Curve {
                spatial,
                first_uv,
                second_uv,
            } => {
                for (surface, uv) in [(first, &first_uv), (second, &second_uv)] {
                    if surface
                        .parameter_curve_deviation_bound(uv, &spatial, limit)?
                        .is_none()
                    {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                }
                if !curves.contains(&spatial) {
                    curves.push(spatial);
                }
            }
        }
    }
    let mut isolated = Vec::new();
    for point in points {
        let mut covered = false;
        for curve in &curves {
            let parameter = curve.closest_parameter(point, tolerance)?;
            if curve.evaluate(parameter)?.distance_to(point)? <= limit {
                covered = true;
                break;
            }
        }
        if !covered {
            isolated.push(point);
        }
    }
    isolated.sort_by(|a, b| compare_points(*a, *b));
    Ok(curves
        .into_iter()
        .map(SurfaceSurfaceIntersectionEvent::Curve)
        .chain(
            isolated
                .into_iter()
                .map(SurfaceSurfaceIntersectionEvent::Point),
        )
        .collect())
}

#[cfg(test)]
mod tests;
