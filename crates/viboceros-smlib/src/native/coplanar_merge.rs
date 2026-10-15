//! Planar face coalescing through the kernel's protected topology remover.
use super::*;
unsafe extern "C" {
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
