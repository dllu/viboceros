//! General coupled XYZ/UV surface intersections with original-surface certificates.
use super::*;
use viboceros_geometry::{
    AffineTransform3, NurbsCurve2, Point2, SurfaceIntersectionInput, SurfaceIntersectionOutput,
    SurfaceSurfaceIntersectionEvent, SurfaceSurfaceIntersectionProposal, WeightedPoint2,
    surface_face_intersection_events_with_operations,
    surface_surface_intersection_events_with_operation,
};
#[repr(C)]
struct NativeEvents {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vb_surface_intersect(
        first: *const brep::NurbsData,
        second: *const brep::NurbsData,
        tolerance: f64,
        angle: f64,
        out: *mut *mut NativeEvents,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_surface_intersections_count(events: *const NativeEvents) -> usize;
    fn vb_surface_intersections_info(
        events: *const NativeEvents,
        index: usize,
        kind: *mut c_int,
        point: *mut f64,
    ) -> c_int;
    fn vb_surface_intersections_take_curve(
        events: *mut NativeEvents,
        index: usize,
        part: usize,
        out: *mut *mut brep::NativeSurfaceData,
    ) -> c_int;
    fn vb_surface_intersections_free(events: *mut NativeEvents);
}
struct Events(NonNull<NativeEvents>);
impl Drop for Events {
    fn drop(&mut self) {
        unsafe {
            vb_surface_intersections_free(self.0.as_ptr());
        }
    }
}

/// Native general surface contacts. Each curve must pass continuous original
/// source/UV correspondence proofs; unsupported preparation retains the portable
/// producer. Native solve and certificate failures propagate.
pub fn surface_intersection_events(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, Error> {
    surface_surface_intersection_events_with_operation(
        first,
        second,
        tolerance,
        &mut |a, b, limit| propose(a, b, tolerance, limit),
    )
}

fn uv_original(curve: NurbsCurve, source: &NurbsSurface) -> Result<NurbsCurve2, Error> {
    let u = source.domain_u();
    let v = source.domain_v();
    let points = curve
        .control_points()
        .iter()
        .map(|p| {
            let q = p.point();
            let x = (*u.start()).mul_add(1. - q.x(), *u.end() * q.x());
            let y = (*v.start()).mul_add(1. - q.y(), *v.end() * q.y());
            WeightedPoint2::try_new(Point2::try_new(x, y)?, p.weight())
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    Ok(NurbsCurve2::try_new_rational(
        curve.degree(),
        points,
        curve.knots().to_vec(),
    )?)
}

fn take(events: &Events, index: usize, part: usize) -> Result<NurbsCurve, Error> {
    let mut pointer = std::ptr::null_mut();
    if unsafe { vb_surface_intersections_take_curve(events.0.as_ptr(), index, part, &mut pointer) }
        != 0
    {
        return Err(Error::InvalidOutput);
    }
    brep::curve_from_native(pointer)
}

fn propose(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
    limit: f64,
) -> Result<Option<Vec<SurfaceSurfaceIntersectionProposal>>, Error> {
    if first.degree_u() > 16
        || first.degree_v() > 16
        || second.degree_u() > 16
        || second.degree_v() > 16
    {
        return Ok(None);
    }
    let origin = first.evaluate(*first.domain_u().start(), *first.domain_v().start())?;
    let mut scale = 0_f64;
    for p in first.control_points().iter().chain(second.control_points()) {
        scale = scale.max(origin.distance_to(p.point())?);
    }
    if !scale.is_finite() || scale <= 0. {
        return Ok(None);
    }
    let Some(a) = super::curve_surface_intersections::prepared_surface(first, origin, scale)?
    else {
        return Ok(None);
    };
    let Some(b) = super::curve_surface_intersections::prepared_surface(second, origin, scale)?
    else {
        return Ok(None);
    };
    let a = knot_refinement::NurbsInput::surface(&a);
    let b = knot_refinement::NurbsInput::surface(&b);
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_surface_intersect(
            &a.view(),
            &b.view(),
            limit / scale,
            tolerance.angular().max(0.1),
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let events = Events(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
    let count = unsafe { vb_surface_intersections_count(events.0.as_ptr()) };
    if count > 500_000 {
        return Err(Error::InvalidOutput);
    }
    let transform = AffineTransform3::try_new(
        [[scale, 0., 0.], [0., scale, 0.], [0., 0., scale]],
        Vector3::try_from(origin.to_array())?,
    )?;
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        let mut kind = 0;
        let mut point = [0.; 3];
        if unsafe {
            vb_surface_intersections_info(events.0.as_ptr(), index, &mut kind, point.as_mut_ptr())
        } != 0
        {
            return Err(Error::InvalidOutput);
        }
        if kind == 1 {
            output.push(SurfaceSurfaceIntersectionProposal::Point(
                transform.transform_point(Point3::try_from(point)?)?,
            ));
        } else {
            if !(2..=6).contains(&kind) {
                return Err(Error::InvalidOutput);
            }
            let spatial = take(&events, index, 0)?.transformed(transform)?;
            let first_uv = uv_original(take(&events, index, 1)?, first)?;
            let second_uv = uv_original(take(&events, index, 2)?, second)?;
            output.push(SurfaceSurfaceIntersectionProposal::Curve {
                spatial,
                first_uv,
                second_uv,
            });
        }
    }
    Ok(Some(output))
}

/// Preserve measured analytic definitions; general curved pairs enter the
/// native producer when the portable analytic dispatcher has no implementation.
pub fn modeling_surface_intersection_events(
    first: &NurbsSurface,
    second: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, Error> {
    match viboceros_geometry::surface_surface_intersection_events(first, second, tolerance) {
        Ok(events) => Ok(events),
        // Specific analytic restrictions include measured coincidence and
        // singularity policies. Only the final general-dispatch gap is extended.
        Err(GeometryError::UnsupportedSurfaceSurfaceIntersection {
            context: "non-planar surfaces",
        }) => surface_intersection_events(first, second, tolerance),
        Err(error) => Err(error.into()),
    }
}

pub fn surface_brep_intersection_events(
    surface: &NurbsSurface,
    brep: &viboceros_geometry::Brep,
    tolerance: Tolerance,
) -> Result<Vec<viboceros_geometry::SurfaceBrepIntersectionEvent>, Error> {
    viboceros_geometry::surface_brep_intersection_events_with_operation(
        surface,
        brep,
        tolerance,
        &mut face_pair_events,
    )
}
pub fn brep_intersection_events(
    first: &viboceros_geometry::Brep,
    second: &viboceros_geometry::Brep,
    tolerance: Tolerance,
) -> Result<Vec<viboceros_geometry::BrepBrepIntersectionEvent>, Error> {
    viboceros_geometry::brep_brep_intersection_events_with_operation(
        first,
        second,
        tolerance,
        &mut face_pair_events,
    )
}

fn face_pair_events(
    first: SurfaceIntersectionInput<'_>,
    second: SurfaceIntersectionInput<'_>,
    tolerance: Tolerance,
) -> Result<SurfaceIntersectionOutput, Error> {
    match viboceros_geometry::surface_surface_intersection_events(
        first.surface(),
        second.surface(),
        tolerance,
    ) {
        Ok(events) => Ok(SurfaceIntersectionOutput::Underlying(events)),
        Err(
            error @ GeometryError::UnsupportedSurfaceSurfaceIntersection {
                context: "non-planar surfaces",
            },
        ) => {
            let Some(events) = surface_face_intersection_events_with_operations(
                first,
                second,
                tolerance,
                &mut |a, b, limit| propose(a, b, tolerance, limit),
                &mut super::curve_intersection_events,
            )?
            else {
                return Err(error.into());
            };
            Ok(SurfaceIntersectionOutput::Clipped(events))
        }
        Err(error) => Err(error.into()),
    }
}
