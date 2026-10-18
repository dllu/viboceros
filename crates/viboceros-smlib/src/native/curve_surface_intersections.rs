//! Native curve/surface proposals, restored and checked on original geometry.
use super::working_curve::{normalized, normalized_controls};
use super::*;
use viboceros_geometry::{
    CurveSurfaceIntersectionEvent, CurveSurfaceIntersectionParameters, SurfaceKnotDirection,
    curve_surface_intersection_events_with_operation,
};

/// Intersect with trimmed faces, retaining shared UV containment and edge policy.
pub fn curve_brep_intersection_events(
    curve: &NurbsCurve,
    brep: &viboceros_geometry::Brep,
    transform: Option<viboceros_geometry::AffineTransform3>,
    tolerance: Tolerance,
) -> Result<Vec<viboceros_geometry::CurveBrepIntersectionEvent>, Error> {
    viboceros_geometry::curve_brep_intersection_events_with_operations(
        curve,
        brep,
        transform,
        tolerance,
        &mut curve_surface_intersection_events,
        &mut super::curve_intersection_events,
    )
}
#[repr(C)]
struct NativeEvents {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NativeEvent {
    overlap: c_int,
    curve: [f64; 2],
    u: [f64; 2],
    v: [f64; 2],
}
unsafe extern "C" {
    fn vb_curve_surface_intersect(
        curve: *const NativeCurve,
        surface: *const brep::NurbsData,
        tolerance: f64,
        out: *mut *mut NativeEvents,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_surface_intersections_count(events: *const NativeEvents) -> usize;
    fn vb_curve_surface_intersections_copy(
        events: *const NativeEvents,
        out: *mut NativeEvent,
        count: usize,
    ) -> c_int;
    fn vb_curve_surface_intersections_free(events: *mut NativeEvents);
}
struct Events(NonNull<NativeEvents>);
impl Drop for Events {
    fn drop(&mut self) {
        unsafe {
            vb_curve_surface_intersections_free(self.0.as_ptr());
        }
    }
}

pub fn curve_surface_intersection_events(
    curve: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<CurveSurfaceIntersectionEvent>, Error> {
    curve_surface_intersection_events_with_operation(curve, surface, tolerance, &mut propose)
}

pub(super) fn prepared_surface(
    source: &NurbsSurface,
    origin: Point3,
    scale: f64,
) -> Result<Option<NurbsSurface>, Error> {
    if source.degree_u() > 17 || source.degree_v() > 17 {
        return Ok(None);
    }
    for (degree, count, knots) in [
        (
            source.degree_u(),
            source.control_point_count_u(),
            source.knots_u(),
        ),
        (
            source.degree_v(),
            source.control_point_count_v(),
            source.knots_v(),
        ),
    ] {
        if knots[degree + 1..count]
            .windows(degree + 1)
            .any(|group| group.iter().all(|k| *k == group[0]))
        {
            return Ok(None);
        }
    }
    let Some(points) = normalized_controls(source.control_points(), origin, scale)? else {
        return Ok(None);
    };
    let mut result = NurbsSurface::try_new_rational(
        source.degree_u(),
        source.degree_v(),
        source.control_point_count_u(),
        source.control_point_count_v(),
        points,
        source.knots_u().to_vec(),
        source.knots_v().to_vec(),
    )?
    .try_reparameterized(0. ..=1., 0. ..=1.)?;
    for (before, after) in [
        (source.knots_u(), result.knots_u()),
        (source.knots_v(), result.knots_v()),
    ] {
        // Reparameterization reconstructs the two artificial full-vector
        // endpoints from the stored OpenNURBS short basis. Only intervals
        // between stored knots can affect the active geometric image.
        if before[1..before.len() - 1]
            .windows(2)
            .zip(after[1..after.len() - 1].windows(2))
            .any(|(a, b)| a[0] < a[1] && b[0] >= b[1])
        {
            return Ok(None);
        }
    }
    // Native surface intersection requires an area image. View projection can
    // collapse a surface to a line or point; keep the portable query for those.
    let points = result.control_points();
    let first = points[0].point();
    let mut axis = None;
    for p in &points[1..] {
        if let Ok(direction) = first.vector_to(p.point())?.normalized_nonzero() {
            axis = Some(direction);
            break;
        }
    }
    let Some(axis) = axis else { return Ok(None) };
    let mut area = false;
    for p in &points[1..] {
        if axis
            .as_vector()
            .cross(first.vector_to(p.point())?)?
            .length()?
            > 128. * f64::EPSILON
        {
            area = true;
            break;
        }
    }
    if !area {
        return Ok(None);
    }
    for (direction, degree, knots) in [
        (
            SurfaceKnotDirection::U,
            result.degree_u(),
            result.knots_u().to_vec(),
        ),
        (
            SurfaceKnotDirection::V,
            result.degree_v(),
            result.knots_v().to_vec(),
        ),
    ] {
        if !knots[..=degree].iter().all(|k| *k == 0.)
            || !knots[knots.len() - degree - 1..].iter().all(|k| *k == 1.)
        {
            result = knot_refinement::clamp_surface_direction(&result, direction)?;
        }
    }
    Ok(Some(result))
}

fn propose(
    source: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: f64,
) -> Result<Option<Vec<CurveSurfaceIntersectionParameters>>, Error> {
    let origin = source.evaluate(*source.domain().start())?;
    let mut scale = 0_f64;
    for p in source
        .control_points()
        .iter()
        .chain(surface.control_points())
    {
        scale = scale.max(origin.distance_to(p.point())?);
    }
    if !scale.is_finite() || scale <= 0. {
        return Ok(None);
    }
    let Some(curve) = normalized(source, origin, scale)? else {
        return Ok(None);
    };
    let Some(prepared) = prepared_surface(surface, origin, scale)? else {
        return Ok(None);
    };
    let curve = KernelCurve::from_nurbs(&curve)?;
    let input = knot_refinement::NurbsInput::surface(&prepared);
    let view = input.view();
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curve_surface_intersect(
            curve.handle.as_ptr(),
            &view,
            tolerance / scale,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let events = Events(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
    let count = unsafe { vb_curve_surface_intersections_count(events.0.as_ptr()) };
    if count > 500_000 {
        return Err(Error::InvalidOutput);
    }
    let mut output = vec![NativeEvent::default(); count];
    if unsafe { vb_curve_surface_intersections_copy(events.0.as_ptr(), output.as_mut_ptr(), count) }
        != 0
    {
        return Err(Error::InvalidOutput);
    }
    let parameter = |t: f64, u: f64, v: f64| -> Result<[f64; 3], Error> {
        if ![t, u, v].iter().all(|p| (0. ..=1.).contains(p)) {
            return Err(Error::InvalidOutput);
        }
        Ok([
            source.parameter_at(t)?,
            surface.parameter_at_u(u)?,
            surface.parameter_at_v(v)?,
        ])
    };
    output
        .into_iter()
        .map(|event| {
            let start = parameter(event.curve[0], event.u[0], event.v[0])?;
            let end = match event.overlap {
                0 => None,
                1 => Some(parameter(event.curve[1], event.u[1], event.v[1])?),
                _ => return Err(Error::InvalidOutput),
            };
            Ok(CurveSurfaceIntersectionParameters { start, end })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    #[test]
    fn unclamped_outer_padding_survives_native_preparation_without_a_fallback() {
        for gauge in [-1e280, 1e-280] {
            let source = NurbsSurface::try_new_rational(
                2,
                1,
                3,
                2,
                [0., 10.]
                    .into_iter()
                    .flat_map(|y| {
                        [-5., 5., 15.].into_iter().map(move |x| {
                            WeightedPoint3::try_new(Point3::try_new(x, y, 0.).unwrap(), gauge)
                                .unwrap()
                        })
                    })
                    .collect(),
                [-2., -1., 0., 1., 2., 3.]
                    .map(|k| k * 2_f64.powi(-360))
                    .to_vec(),
                vec![1e12, 1e12, 1e12 + 10., 1e12 + 10.],
            )
            .unwrap();
            let before = source.clone();
            let origin = Point3::try_new(0., 0., 0.).unwrap();
            let result = prepared_surface(&source, origin, 10.)
                .unwrap()
                .expect("valid unclamped source must enter the native producer");
            assert_eq!(result.domain_u(), 0. ..=1.);
            assert_eq!(result.domain_v(), 0. ..=1.);
            for i in 0..=8 {
                for j in 0..=8 {
                    let a = result.evaluate(i as f64 / 8., j as f64 / 8.).unwrap();
                    let expected = Point3::try_new(i as f64 / 8., j as f64 / 8., 0.).unwrap();
                    assert!(a.distance_to(expected).unwrap() < 1e-12);
                }
            }
            assert_eq!(source, before);
        }
    }
}
