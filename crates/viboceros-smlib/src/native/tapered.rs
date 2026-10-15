//! Draft sweeps delegate offsets, corner construction and topology to SMLib.
use super::*;

unsafe extern "C" {
    fn vb_shape_tapered_extrusion(
        curve: *const NativeCurve,
        direction: *const f64,
        height: f64,
        angle_degrees: f64,
        round_corners: c_int,
        cap_ends: c_int,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

impl Shape {
    /// Extrude a connected planar profile with a signed draft angle in degrees.
    /// Positive angles shrink a closed profile; negative angles expand it.
    /// `round_corners` requests native fillets instead of sharp offset joints.
    /// Nonzero draft requires a planar profile and extrusion normal to its plane.
    /// Source geometry is copied. A requested cap must produce a manifold solid.
    pub fn tapered_extrusion(
        profile: &KernelCurve,
        displacement: viboceros_geometry::Vector3,
        angle_degrees: f64,
        round_corners: bool,
        cap_ends: bool,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Self, Error> {
        if !angle_degrees.is_finite() || angle_degrees.abs() >= 90. {
            return Err(Error::Kernel(
                "Draft angle must be finite and between -90 and 90 degrees".into(),
            ));
        }
        if angle_degrees == 0. {
            return Self::linear_sweep(&[profile], displacement, cap_ends, tolerance);
        }
        let height = displacement.length()?;
        let direction = displacement.normalized_nonzero()?.as_vector();
        if let Some(normal) = KernelCurve::parallel_plane_normal(&[profile], tolerance)?
            && direction.cross(normal)?.length()? > tolerance.angular()
        {
            return Err(Error::Kernel(
                "Nonzero draft currently requires extrusion normal to the profile plane".into(),
            ));
        }
        let direction = direction.to_array();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_tapered_extrusion(
                profile.handle.as_ptr(),
                direction.as_ptr(),
                height,
                angle_degrees,
                c_int::from(round_corners),
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
