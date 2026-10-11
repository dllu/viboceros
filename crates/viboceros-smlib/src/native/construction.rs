//! Existing native surface and B-rep producers with owned NURBS transfer.
use super::*;

unsafe extern "C" {
    fn vb_curve_extrude(
        curve: *const NativeCurve,
        displacement: *const f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_linear_sweep(
        curves: *const *const NativeCurve,
        count: usize,
        displacement: *const f64,
        cap_ends: c_int,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

impl KernelCurve {
    /// Produce an exact native extrusion surface; this curve is unchanged.
    /// The returned surface owns its NURBS data and contains no native pointers.
    pub fn extruded_surface(&self, displacement: Vector3) -> Result<NurbsSurface, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_curve_extrude(
                self.handle.as_ptr(),
                displacement.to_array().as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        brep::surface_from_native(pointer)
    }
}

impl Shape {
    /// Sweep private copies of profiles into sheets or capped solid regions.
    /// Profile handles and their geometry remain owned by the caller.
    pub fn linear_sweep(
        curves: &[&KernelCurve],
        displacement: Vector3,
        cap_ends: bool,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        let handles = curves
            .iter()
            .map(|c| c.handle.as_ptr().cast_const())
            .collect::<Vec<_>>();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_linear_sweep(
                handles.as_ptr(),
                handles.len(),
                displacement.to_array().as_ptr(),
                c_int::from(cap_ends),
                tolerance.absolute(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            model: Solid {
                handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
                _thread_local: PhantomData,
            },
        })
    }
}
