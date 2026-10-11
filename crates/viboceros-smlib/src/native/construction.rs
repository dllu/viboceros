//! Existing native surface and B-rep producers with owned NURBS transfer.
use super::*;

unsafe extern "C" {
    fn vb_curve_translational_sweep(
        profile: *const NativeCurve,
        path: *const NativeCurve,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_translational_sweep(
        profile: *const NativeCurve,
        path: *const NativeCurve,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_apex_surface(
        curve: *const NativeCurve,
        apex: *const f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_apex_solid(
        curve: *const NativeCurve,
        apex: *const f64,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curves_coplanar(
        curves: *const *const NativeCurve,
        count: usize,
        tolerance: f64,
        coplanar: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_planar_faces(
        curves: *const *const NativeCurve,
        count: usize,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_revolve(
        curve: *const NativeCurve,
        origin: *const f64,
        axis: *const f64,
        angle_degrees: f64,
        parameter_length: f64,
        out: *mut *mut brep::NativeSurfaceData,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
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
    /// Exact fixed-orientation sweep: profile(u) + path(v) - path(start).
    /// U/V retain profile/path parameters. Both input handles remain unchanged.
    pub fn translational_sweep(&self, path: &Self) -> Result<NurbsSurface, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_curve_translational_sweep(
                self.handle.as_ptr(),
                path.handle.as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        brep::surface_from_native(pointer)
    }
    /// Create an exact native ruled surface from this profile to an apex.
    /// The collapsed boundary retains the profile weights; source is unchanged.
    pub fn apex_surface(&self, apex: Point3) -> Result<NurbsSurface, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_curve_apex_surface(
                self.handle.as_ptr(),
                apex.to_array().as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        brep::surface_from_native(pointer)
    }
    /// Query the entire profile group with the native plane classifier.
    pub fn are_coplanar(curves: &[&Self], tolerance: Tolerance) -> Result<bool, Error> {
        let handles = curves
            .iter()
            .map(|c| c.handle.as_ptr().cast_const())
            .collect::<Vec<_>>();
        let mut plane = 0;
        call(|error, capacity| unsafe {
            vb_curves_coplanar(
                handles.as_ptr(),
                handles.len(),
                tolerance.absolute(),
                &mut plane,
                error,
                capacity,
            )
        })?;
        Ok(plane != 0)
    }
    /// Import a modeling profile using native active-interval clamping.
    /// Uniformly negative weights are flipped as one homogeneous gauge.
    /// Mixed signs retain the strict importer rejection; source data is unchanged.
    pub fn from_nurbs_for_modeling(curve: &NurbsCurve) -> Result<Self, Error> {
        if curve.control_points().iter().all(|p| p.weight() < 0.) {
            let prepared = NurbsCurve::try_new_rational(
                curve.degree(),
                curve
                    .control_points()
                    .iter()
                    .map(|p| WeightedPoint3::try_new(p.point(), -p.weight()))
                    .collect::<Result<Vec<_>, _>>()?,
                curve.knots().to_vec(),
            )?;
            Self::from_nurbs(&prepared)
        } else {
            Self::from_nurbs(curve)
        }
    }

    /// Create a complete native revolution tensor, circular U and profile V.
    /// The angular sweep is positive and at most 360 degrees; reverse the axis
    /// for a negative sweep. `parameter_length` specifies the circular U domain.
    pub fn revolved_surface(
        &self,
        origin: Point3,
        axis: Vector3,
        angle_degrees: f64,
        parameter_length: f64,
    ) -> Result<NurbsSurface, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_curve_revolve(
                self.handle.as_ptr(),
                origin.to_array().as_ptr(),
                axis.to_array().as_ptr(),
                angle_degrees,
                parameter_length,
                &mut pointer,
                error,
                capacity,
            )
        })?;
        brep::surface_from_native(pointer)
    }
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
    /// Cap and stitch the native exact sweep of a closed planar profile along
    /// an open path. The path endpoint displacement must leave the profile plane.
    pub fn translational_sweep(
        profile: &KernelCurve,
        path: &KernelCurve,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_translational_sweep(
                profile.handle.as_ptr(),
                path.handle.as_ptr(),
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
    /// Construct a capped native apex solid from a closed planar profile copy.
    pub fn apex_solid(
        curve: &KernelCurve,
        apex: Point3,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_apex_solid(
                curve.handle.as_ptr(),
                apex.to_array().as_ptr(),
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
    /// Construct native coplanar regions, including holes and nested islands.
    /// The kernel consumes private profile copies; caller-owned curves remain usable.
    pub fn planar_faces(curves: &[&KernelCurve], tolerance: Tolerance) -> Result<Self, Error> {
        let handles = curves
            .iter()
            .map(|c| c.handle.as_ptr().cast_const())
            .collect::<Vec<_>>();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_planar_faces(
                handles.as_ptr(),
                handles.len(),
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
