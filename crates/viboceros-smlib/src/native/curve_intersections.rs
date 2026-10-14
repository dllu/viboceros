//! Native point/range proposals with independent original-curve acceptance.
use super::working_curve::normalized;
use super::*;
use viboceros_geometry::{CurveCurveIntersectionEvent, CurveCurveIntersectionParameters};

#[repr(C)]
struct NativeEvents {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NativeEvent {
    overlap: c_int,
    first: [f64; 2],
    second: [f64; 2],
}
unsafe extern "C" {
    fn vb_curve_intersect(
        first: *const NativeCurve,
        second: *const NativeCurve,
        tolerance: f64,
        out: *mut *mut NativeEvents,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_intersections_count(events: *const NativeEvents) -> usize;
    fn vb_curve_self_intersect(
        curve: *const NativeCurve,
        tolerance: f64,
        out: *mut *mut NativeEvents,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_intersections_copy(
        events: *const NativeEvents,
        out: *mut NativeEvent,
        count: usize,
    ) -> c_int;
    fn vb_curve_intersections_free(events: *mut NativeEvents);
}
struct Events(NonNull<NativeEvents>);
impl Drop for Events {
    fn drop(&mut self) {
        unsafe {
            vb_curve_intersections_free(self.0.as_ptr());
        }
    }
}

pub fn curve_intersection_events(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Vec<CurveCurveIntersectionEvent>, Error> {
    first.intersection_events_with_curve_with_operation(second, tolerance, &mut propose)
}

/// Contacts between distinct visits of one curve. A closed endpoint seam is
/// excluded. `None` declines inputs outside the native working-frame limits.
/// Point residuals and sampled overlap images are checked on the original curve;
/// these checks do not certify completeness or continuous coincidence.
pub fn curve_self_intersection_events(
    source: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<Option<Vec<CurveCurveIntersectionEvent>>, Error> {
    source.tight_bounds(tolerance)?;
    let origin = source.evaluate(*source.domain().start())?;
    let mut scale = 0_f64;
    for control in source.control_points() {
        scale = scale.max(origin.distance_to(control.point())?);
    }
    if !scale.is_finite() || scale <= 0. {
        return Ok(None);
    }
    let Some(prepared) = normalized(source, origin, scale)? else {
        return Ok(None);
    };
    let curve = KernelCurve::from_nurbs(&prepared)?;
    let events = source.intersection_events_with_curve_with_operation(
        source,
        tolerance,
        &mut |_, _, limit| -> Result<_, Error> {
            let mut pointer = std::ptr::null_mut();
            call(|error, capacity| unsafe {
                vb_curve_self_intersect(
                    curve.handle.as_ptr(),
                    limit / scale,
                    &mut pointer,
                    error,
                    capacity,
                )
            })?;
            let mut parameters = copy_parameters(source, source, pointer)?;
            if parameters.iter().any(|event| {
                event.start[0] > event.start[1] || event.end.is_some_and(|end| end[0] > end[1])
            }) {
                return Err(Error::InvalidOutput);
            }
            let domain = source.domain();
            let closed = origin.distance_to(source.evaluate(*domain.end())?)? <= limit;
            parameters.retain(|event| {
                if event.end.is_some() {
                    return true;
                }
                let [a, b] = event.start;
                a != b
                    && !(closed
                        && ((a == *domain.start() && b == *domain.end())
                            || (b == *domain.start() && a == *domain.end())))
            });
            Ok(Some(parameters))
        },
    )?;
    Ok(Some(events))
}

fn propose(
    first: &NurbsCurve,
    second: &NurbsCurve,
    tolerance: f64,
) -> Result<Option<Vec<CurveCurveIntersectionParameters>>, Error> {
    let origin = first.evaluate(*first.domain().start())?;
    let mut scale = 0_f64;
    for control in first.control_points().iter().chain(second.control_points()) {
        scale = scale.max(origin.distance_to(control.point())?);
    }
    if !scale.is_finite() || scale <= 0. {
        return Ok(None);
    }
    let Some(a) = normalized(first, origin, scale)? else {
        return Ok(None);
    };
    let Some(b) = normalized(second, origin, scale)? else {
        return Ok(None);
    };
    let a = KernelCurve::from_nurbs(&a)?;
    let b = KernelCurve::from_nurbs(&b)?;
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curve_intersect(
            a.handle.as_ptr(),
            b.handle.as_ptr(),
            tolerance / scale,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    copy_parameters(first, second, pointer).map(Some)
}

fn copy_parameters(
    first: &NurbsCurve,
    second: &NurbsCurve,
    pointer: *mut NativeEvents,
) -> Result<Vec<CurveCurveIntersectionParameters>, Error> {
    let events = Events(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
    let count = unsafe { vb_curve_intersections_count(events.0.as_ptr()) };
    if count > 500_000 {
        return Err(Error::InvalidOutput);
    }
    let mut output = vec![NativeEvent::default(); count];
    if unsafe { vb_curve_intersections_copy(events.0.as_ptr(), output.as_mut_ptr(), count) } != 0 {
        return Err(Error::InvalidOutput);
    }
    let parameter = |curve: &NurbsCurve, t: f64| -> Result<f64, Error> {
        if !(0. ..=1.).contains(&t) {
            return Err(Error::InvalidOutput);
        }
        Ok(curve.parameter_at(t)?)
    };
    output
        .into_iter()
        .map(|event| {
            let start = [
                parameter(first, event.first[0])?,
                parameter(second, event.second[0])?,
            ];
            let end = match event.overlap {
                0 => None,
                1 => Some([
                    parameter(first, event.first[1])?,
                    parameter(second, event.second[1])?,
                ]),
                _ => return Err(Error::InvalidOutput),
            };
            Ok(CurveCurveIntersectionParameters { start, end })
        })
        .collect()
}
