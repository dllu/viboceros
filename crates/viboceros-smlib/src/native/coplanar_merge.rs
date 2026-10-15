//! Planar face coalescing through the kernel's protected topology remover.
use super::*;
unsafe extern "C" {
    fn vb_imported_shape_merge_coplanar_subset(
        shape: *mut NativeSolid,
        indices: *const usize,
        count: usize,
        region: c_int,
        tolerance: f64,
        angle: f64,
        removed: *mut usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_merge_coplanar_faces(
        shape: *const NativeSolid,
        tolerance: f64,
        angle_tolerance: f64,
        removed: *mut usize,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
impl Shape {
    /// Merge only a pair of original B-rep faces, or the planar connected region
    /// containing one original face. The temporary import owns all native edits.
    pub fn merge_brep_coplanar_subset(
        brep: &viboceros_geometry::Brep,
        faces: &[usize],
        region: bool,
        tolerance: Tolerance,
    ) -> Result<Option<(Self, usize)>, Error> {
        if faces.len() != if region { 1 } else { 2 }
            || faces.iter().any(|&i| i >= brep.faces().len())
            || (!region && faces[0] == faces[1])
        {
            return Err(Error::Kernel(
                "Invalid selected coplanar face indices".into(),
            ));
        }
        let shape = Self::from_brep(brep, tolerance)?;
        let mut removed = 0;
        call(|error, capacity| unsafe {
            vb_imported_shape_merge_coplanar_subset(
                shape.model.handle.as_ptr(),
                faces.as_ptr(),
                faces.len(),
                region.into(),
                tolerance.absolute(),
                tolerance.angular(),
                &mut removed,
                error,
                capacity,
            )
        })?;
        Ok((removed > 0).then_some((shape, removed)))
    }

    /// Merge edge-adjacent planar faces on a private copy. Curved interfaces,
    /// seam edges and retained boundary vertices are protected. None means no merge.
    pub fn merge_coplanar_faces(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<(Self, usize)>, Error> {
        let mut pointer = std::ptr::null_mut();
        let mut removed = 0;
        call(|error, capacity| unsafe {
            vb_shape_merge_coplanar_faces(
                self.model.handle.as_ptr(),
                tolerance.absolute(),
                tolerance.angular(),
                &mut removed,
                &mut pointer,
                error,
                capacity,
            )
        })?;
        let result = NonNull::new(pointer).map(|handle| Self {
            model: Solid {
                handle,
                _thread_local: PhantomData,
            },
        });
        match (removed, result) {
            (0, None) => Ok(None),
            (count, Some(shape)) if count > 0 => Ok(Some((shape, count))),
            _ => Err(Error::InvalidOutput),
        }
    }
}
