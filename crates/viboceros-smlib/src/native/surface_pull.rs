//! Surface-normal curve drops from usd-brep with original-surface image bounds.
use super::*;
use viboceros_geometry::{AffineTransform3, NurbsCurve2};
#[derive(Clone, Debug)]
pub struct SurfacePullCurve {
    pub spatial: NurbsCurve,
    pub uv: NurbsCurve2,
    /// Certified complete spatial-image error against the original surface.
    pub surface_deviation: f64,
}
unsafe extern "C" {
    fn vb_surface_pull(
        curve: *const NativeCurve,
        surface: *const brep::NurbsData,
        tolerance: f64,
        out: *mut *mut surface_intersections::NativeEvents,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
/// Pull to the finite natural surface domain. The native normal-drop solver
/// supplies paired curves; a continuous certificate bounds every lifted point.
/// This does not independently certify global nearest-branch choice.
pub fn pull_curve_to_surface(
    source: &NurbsCurve,
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<Vec<SurfacePullCurve>, Error> {
    let origin = surface.evaluate(*surface.domain_u().start(), *surface.domain_v().start())?;
    let mut scale = 0_f64;
    for p in source
        .control_points()
        .iter()
        .chain(surface.control_points())
    {
        scale = scale.max(origin.distance_to(p.point())?);
    }
    if !scale.is_finite() || scale <= 0. {
        return Err(Error::InvalidOutput);
    }
    let prepared = working_curve::normalized(source, origin, scale)?
        .ok_or_else(|| Error::Kernel("unsupported surface pull source preparation".into()))?;
    let target = curve_surface_intersections::prepared_surface(surface, origin, scale)?
        .ok_or_else(|| Error::Kernel("unsupported surface pull target preparation".into()))?;
    let curve = KernelCurve::from_nurbs(&prepared)?;
    let input = knot_refinement::NurbsInput::surface(&target);
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_surface_pull(
            curve.handle.as_ptr(),
            &input.view(),
            tolerance.absolute() / scale,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let pairs = surface_intersections::paired_curves(pointer)?;
    let transform = AffineTransform3::try_new(
        [[scale, 0., 0.], [0., scale, 0.], [0., 0., scale]],
        Vector3::try_from(origin.to_array())?,
    )?;
    pairs
        .into_iter()
        .map(|(spatial, uv)| {
            let spatial = spatial.transformed(transform)?;
            let uv = surface_intersections::uv_original(uv, surface)?;
            let surface_deviation = surface
                .parameter_curve_deviation_bound(&uv, &spatial, tolerance.absolute())?
                .ok_or_else(|| {
                    Error::Kernel(
                        "surface pull image could not be certified within model tolerance".into(),
                    )
                })?;
            Ok(SurfacePullCurve {
                spatial,
                uv,
                surface_deviation,
            })
        })
        .collect()
}
#[cfg(test)]
mod tests;
