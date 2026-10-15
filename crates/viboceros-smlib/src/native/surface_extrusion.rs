//! Native topology sweep preserves the source faces as curved or trimmed caps.
use super::*;
unsafe extern "C" {
    fn vb_shape_extrude_faces_to_point(
        shape: *const NativeSolid,
        apex: *const f64,
        cap_base: c_int,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_extrude_faces_along_curve(
        shape: *const NativeSolid,
        path: *const NativeCurve,
        cap_ends: c_int,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_single_face_plane_normal(
        shape: *const NativeSolid,
        tolerance: f64,
        has_plane: *mut c_int,
        normal: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_extrude_faces(
        shape: *const NativeSolid,
        displacement: *const f64,
        cap_ends: c_int,
        tolerance: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
impl Shape {
    /// Sweep an open sheet's boundary to an apex using native ruled surfaces.
    /// The optional base cap retains the actual source faces and trim holes.
    /// Source geometry remains unchanged; capped output must close as a solid.
    pub fn extruded_faces_to_point(
        &self,
        apex: Point3,
        cap_base: bool,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_extrude_faces_to_point(
                self.model.handle.as_ptr(),
                apex.to_array().as_ptr(),
                c_int::from(cap_base),
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

    /// Translate an open sheet along a curve while preserving profile orientation.
    /// The source stays at its original position. Caps copy its actual faces;
    /// closed or degenerate paths decline. Both source and path remain unchanged.
    pub fn extruded_faces_along_curve(
        &self,
        path: &KernelCurve,
        cap_ends: bool,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_extrude_faces_along_curve(
                self.model.handle.as_ptr(),
                path.handle.as_ptr(),
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
    /// Query the native plane classifier for a single face; curved faces return None.
    pub fn single_face_plane_normal(
        &self,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Option<viboceros_geometry::Vector3>, Error> {
        let mut has_plane = 0;
        let mut normal = [0.; 3];
        call(|error, capacity| unsafe {
            vb_shape_single_face_plane_normal(
                self.model.handle.as_ptr(),
                tolerance.absolute(),
                &mut has_plane,
                normal.as_mut_ptr(),
                error,
                capacity,
            )
        })?;
        match has_plane {
            0 => Ok(None),
            1 => Ok(Some(
                viboceros_geometry::Vector3::try_from(normal)?
                    .normalized_nonzero()?
                    .as_vector(),
            )),
            _ => Err(Error::InvalidOutput),
        }
    }
    /// Sweep all faces of an open sheet along a fixed displacement.
    /// Native topology construction operates on an independent copy. With caps,
    /// the original face images and their translated copies bound the result;
    /// curved caps are retained. A capped result must be a manifold solid.
    pub fn extruded_faces(
        &self,
        displacement: viboceros_geometry::Vector3,
        cap_ends: bool,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Self, Error> {
        displacement.normalized_nonzero()?;
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_extrude_faces(
                self.model.handle.as_ptr(),
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
