//! Normal-offset fitting of smooth NURBS supports in inherited parameters.
use super::*;

unsafe extern "C" {
    fn vb_nurbs_offset_surface(
        input: *const brep::NurbsData,
        distance: f64,
        approximation: f64,
        output: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Fit a signed normal offset without the B-rep kernel's chart normalization.
/// The caller must partition nonsmooth supports before fitting and validate
/// the assembled topology. The native fit is not a continuous ideal-offset proof.
pub fn offset_surface(
    source: &NurbsSurface,
    distance: f64,
    approximation: f64,
) -> Result<NurbsSurface, Error> {
    let input = knot_refinement::NurbsInput::surface(source);
    let mut pointer = std::ptr::null_mut();
    call(|error, capacity| unsafe {
        vb_nurbs_offset_surface(
            &input.view(),
            distance,
            approximation,
            &mut pointer,
            error,
            capacity,
        )
    })?;
    let result = brep::surface_from_native(pointer)?;
    if result.domain_u() != source.domain_u() || result.domain_v() != source.domain_v() {
        return Err(Error::Kernel(
            "Offset fit changed the source parameter domain".into(),
        ));
    }
    Ok(result)
}
