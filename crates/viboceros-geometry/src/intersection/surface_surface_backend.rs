//! Continuous original-surface acceptance for coupled intersection proposals.
use super::*;
use crate::NurbsCurve2;
mod trim;

/// An underlying surface, optionally restricted by its original face trims.
#[derive(Clone, Copy, Debug)]
pub enum SurfaceIntersectionInput<'a> {
    Surface(&'a NurbsSurface),
    Face(&'a BrepFace),
}
impl<'a> SurfaceIntersectionInput<'a> {
    pub fn surface(self) -> &'a NurbsSurface {
        match self {
            Self::Surface(surface) => surface,
            Self::Face(face) => face.surface(),
        }
    }
}

/// Whether a producer has already restricted its curves to the supplied faces.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceIntersectionOutput {
    Underlying(Vec<SurfaceSurfaceIntersectionEvent>),
    Clipped(Vec<SurfaceSurfaceIntersectionEvent>),
}
impl SurfaceIntersectionOutput {
    pub(super) fn events(self) -> Vec<SurfaceSurfaceIntersectionEvent> {
        match self {
            Self::Underlying(events) | Self::Clipped(events) => events,
        }
    }
}

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
    match surface_face_intersection_events_with_operations(
        SurfaceIntersectionInput::Surface(first),
        SurfaceIntersectionInput::Surface(second),
        tolerance,
        operation,
        &mut |a, b, tol| Ok(a.intersection_events_with_curve(b, tol)?),
    )? {
        Some(events) => Ok(events),
        None => Ok(surface_surface_intersection_events(
            first, second, tolerance,
        )?),
    }
}

/// Accept coupled XYZ/UV proposals, clipping witnesses against original face
/// loops using a supplied curve intersector in normalized UV coordinates.
/// Every emitted spatial span is recertified against both original surfaces.
/// `None` declines the producer; it does not mean an empty intersection.
/// Curve acceptance is continuous, while trim contact discovery and interval
/// classification do not prove completeness or general topology.
pub fn surface_face_intersection_events_with_operations<E: From<GeometryError>>(
    first_input: SurfaceIntersectionInput<'_>,
    second_input: SurfaceIntersectionInput<'_>,
    tolerance: Tolerance,
    operation: &mut impl FnMut(
        &NurbsSurface,
        &NurbsSurface,
        Real,
    ) -> Result<Option<Vec<SurfaceSurfaceIntersectionProposal>>, E>,
    curve_events: &mut impl FnMut(
        &NurbsCurve,
        &NurbsCurve,
        Tolerance,
    ) -> Result<Vec<crate::CurveCurveIntersectionEvent>, E>,
) -> Result<Option<Vec<SurfaceSurfaceIntersectionEvent>>, E> {
    let first = first_input.surface();
    let second = second_input.surface();
    let limit = surface_surface_distance_tolerance(first, second, tolerance);
    let Some(proposals) = operation(first, second, limit)? else {
        return Ok(None);
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
    let regions = [
        trim::Region::new(first_input, tolerance)?,
        trim::Region::new(second_input, tolerance)?,
    ];
    let mut work = 0;
    for proposal in proposals {
        match proposal {
            SurfaceSurfaceIntersectionProposal::Point(point) => {
                let mut contained = true;
                for (surface, region) in [first, second].into_iter().zip(&regions) {
                    let (u, v) = surface.closest_parameters(point, tolerance)?;
                    if surface.evaluate(u, v)?.distance_to(point)? > limit {
                        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                    }
                    if let Some(region) = region {
                        contained &= region.contains_original(u, v)?;
                    }
                }
                // A lone coplanar corner is excluded by the measured command policy.
                if !coplanar && contained {
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
                if regions.iter().any(Option::is_some) {
                    let clipped = trim::clip(
                        &spatial,
                        [&first_uv, &second_uv],
                        &regions,
                        limit,
                        curve_events,
                        &mut work,
                    )?;
                    for point in clipped.points {
                        push_unique_brep_point(&mut points, point, limit);
                    }
                    for (piece, witnesses) in clipped.curves {
                        for (surface, uv) in [first, second].into_iter().zip(&witnesses) {
                            if surface
                                .parameter_curve_deviation_bound(uv, &piece, limit)?
                                .is_none()
                            {
                                return Err(GeometryError::CurveIntersectionDidNotConverge.into());
                            }
                        }
                        if !curves.contains(&piece) {
                            curves.push(piece);
                        }
                    }
                } else if !curves.contains(&spatial) {
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
    Ok(Some(
        curves
            .into_iter()
            .map(SurfaceSurfaceIntersectionEvent::Curve)
            .chain(
                isolated
                    .into_iter()
                    .map(SurfaceSurfaceIntersectionEvent::Point),
            )
            .collect(),
    ))
}

/// Include already-clipped certified payloads without joining their endpoints.
pub(super) fn append_clipped(
    curves: &mut Vec<NurbsCurve>,
    points: &mut Vec<Point3>,
    clipped: Vec<SurfaceSurfaceIntersectionEvent>,
    tolerance: Tolerance,
    limit: Real,
) -> Result<(), GeometryError> {
    if clipped.is_empty() {
        return Ok(());
    }
    for event in clipped {
        match event {
            SurfaceSurfaceIntersectionEvent::Curve(curve) => {
                if !curves.contains(&curve) {
                    curves.push(curve);
                }
            }
            SurfaceSurfaceIntersectionEvent::Point(point) => {
                push_unique_brep_point(points, point, limit)
            }
        }
    }
    let mut isolated = Vec::new();
    for &point in points.iter() {
        let mut covered = false;
        for curve in curves.iter() {
            let t = curve.closest_parameter(point, tolerance)?;
            if curve.evaluate(t)?.distance_to(point)? <= limit {
                covered = true;
                break;
            }
        }
        if !covered {
            isolated.push(point);
        }
    }
    *points = isolated;
    Ok(())
}

#[cfg(test)]
mod tests;

/// Restrict a paired spatial/UV curve to a face using the shared trim-region
/// clipper. Every retained curve is certified on the original surface.
pub fn clip_surface_curve_to_face_with_operation<E: From<GeometryError>>(
    spatial: &NurbsCurve,
    uv: &NurbsCurve2,
    face: &crate::BrepFace,
    tolerance: Tolerance,
    curve_events: &mut impl FnMut(
        &NurbsCurve,
        &NurbsCurve,
        Tolerance,
    ) -> Result<Vec<crate::CurveCurveIntersectionEvent>, E>,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, E> {
    let surface = face.surface();
    let limit = tolerance.absolute();
    if surface
        .parameter_curve_deviation_bound(uv, spatial, limit)?
        .is_none()
    {
        return Err(GeometryError::CurveIntersectionDidNotConverge.into());
    }
    let region = trim::Region::new(SurfaceIntersectionInput::Face(face), tolerance)?;
    if region.is_none() {
        return Ok(vec![SurfaceSurfaceIntersectionEvent::Curve(
            spatial.clone(),
        )]);
    }
    let clipped = trim::clip(
        spatial,
        [uv, uv],
        &[region, None],
        limit,
        curve_events,
        &mut 0,
    )?;
    let mut result = Vec::new();
    for (piece, witnesses) in clipped.curves {
        if surface
            .parameter_curve_deviation_bound(&witnesses[0], &piece, limit)?
            .is_none()
        {
            return Err(GeometryError::CurveIntersectionDidNotConverge.into());
        }
        result.push(SurfaceSurfaceIntersectionEvent::Curve(piece));
    }
    // Interior span endpoints are already represented by those curves. Keep
    // only isolated contacts, following the shared intersection event policy.
    for point in clipped.points {
        if !result.iter().any(|event| match event {
            SurfaceSurfaceIntersectionEvent::Curve(c) => c
                .closest_parameter(point, tolerance)
                .and_then(|t| c.evaluate(t))
                .is_ok_and(|p| p.distance_to(point).is_ok_and(|d| d <= limit)),
            SurfaceSurfaceIntersectionEvent::Point(p) => {
                p.distance_to(point).is_ok_and(|d| d <= limit)
            }
        }) {
            result.push(SurfaceSurfaceIntersectionEvent::Point(point));
        }
    }
    Ok(result)
}
