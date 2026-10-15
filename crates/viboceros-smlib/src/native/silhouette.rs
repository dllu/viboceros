//! Trimmed parallel silhouettes produced by SMLib's hidden-curve recovery.
use super::*;

#[repr(C)]
struct NativeCurves {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vb_shape_silhouettes(
        shape: *const NativeSolid,
        direction: *const f64,
        approximation: f64,
        angle: f64,
        internal_edges: c_int,
        out: *mut *mut NativeCurves,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_list_count(curves: *const NativeCurves) -> usize;
    fn vb_curve_list_take(
        curves: *mut NativeCurves,
        index: usize,
        out: *mut *mut brep::NativeSurfaceData,
    ) -> c_int;
    fn vb_curve_list_free(curves: *mut NativeCurves);
}
struct Curves(NonNull<NativeCurves>);
impl Drop for Curves {
    fn drop(&mut self) {
        unsafe { vb_curve_list_free(self.0.as_ptr()) };
    }
}

impl Shape {
    /// Spatial curves visible in a parallel view, directed from the model
    /// toward the viewer. SMLib owns its working scene; this shape is unchanged.
    /// `internal_edges` also enables visible seams and smooth/feature edges.
    /// The supplied distances control native tracing and final NURBS conversion;
    /// they are not a continuous geometric error certificate.
    pub fn parallel_silhouette_curves(
        &self,
        toward_eye: viboceros_geometry::Vector3,
        approximation: f64,
        angular_tolerance: f64,
        internal_edges: bool,
    ) -> Result<Vec<NurbsCurve>, Error> {
        if !approximation.is_finite()
            || approximation <= 0.
            || approximation * 0.5 == 0.
            || !angular_tolerance.is_finite()
            || angular_tolerance <= 0.
            || angular_tolerance > std::f64::consts::PI
        {
            return Err(Error::Kernel(
                "Silhouette tolerances must be finite and positive; angle must be at most pi"
                    .into(),
            ));
        }
        let direction = toward_eye.normalized_nonzero()?.as_vector().to_array();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_silhouettes(
                self.model.handle.as_ptr(),
                direction.as_ptr(),
                approximation,
                angular_tolerance,
                c_int::from(internal_edges),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        let curves = Curves(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
        let count = unsafe { vb_curve_list_count(curves.0.as_ptr()) };
        if count > 1_000_000 {
            return Err(Error::InvalidOutput);
        }
        (0..count)
            .map(|index| {
                let mut pointer = std::ptr::null_mut();
                if unsafe { vb_curve_list_take(curves.0.as_ptr(), index, &mut pointer) } != 0 {
                    return Err(Error::InvalidOutput);
                }
                brep::curve_from_native(pointer)
            })
            .collect()
    }
}
