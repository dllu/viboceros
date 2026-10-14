//! Pointwise Hermite approximation of native offset evaluators.
use super::working_curve::normalized;
use super::*;
use viboceros_geometry::{AffineTransform3, UnitVector3, Vector3};

unsafe extern "C" {
    fn vb_curve_offset_approx(
        source: *const NativeCurve,
        normal: *const f64,
        distance: f64,
        tolerance: f64,
        out: *mut *mut NativeCurve,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

pub(super) fn fit(
    source: &NurbsCurve,
    distance: f64,
    normal: UnitVector3,
    tolerance: Tolerance,
    closed: bool,
) -> Result<Option<NurbsCurve>, Error> {
    let origin = source.evaluate(*source.domain().start())?;
    let mut scale = distance.abs();
    for p in source.control_points() {
        scale = scale.max(origin.distance_to(p.point())?);
    }
    if !scale.is_finite() || scale <= 0. {
        return Ok(None);
    }
    // The native evaluator treats smaller normalized offsets as zero.
    if distance.abs() / scale < 1e-12 {
        return Ok(None);
    }
    let Some(prepared) = normalized(source, origin, scale)? else {
        return Ok(None);
    };
    let imported = KernelCurve::from_nurbs(&prepared)?;
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_curve_offset_approx(
            imported.handle.as_ptr(),
            normal.as_vector().to_array().as_ptr(),
            distance / scale,
            tolerance.absolute() * 0.125 / scale,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let result = KernelCurve {
        handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
        _thread_local: PhantomData,
    };
    let result = result.to_nurbs()?;
    let restore = AffineTransform3::try_uniform_scale(Point3::try_new(0., 0., 0.)?, scale)?.then(
        AffineTransform3::from_translation(Vector3::try_from(origin.to_array())?),
    )?;
    let mut result = result
        .transformed(restore)?
        .try_reparameterized(source.domain())?;
    if closed {
        let mut controls = result.control_points().to_vec();
        let last = controls.len() - 1;
        controls[last] = WeightedPoint3::try_new(controls[0].point(), controls[last].weight())?;
        result = NurbsCurve::try_new_rational(result.degree(), controls, result.knots().to_vec())?;
    }
    Ok(Some(result))
}
