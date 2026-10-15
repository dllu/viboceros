//! Solid shelling through the native offset/intersection and Boolean executive.
use super::*;
unsafe extern "C" {
    fn vb_shape_shell_faces(
        shape: *const NativeSolid,
        indices: *const usize,
        count: usize,
        thickness: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
impl Shape {
    /// Hollow an imported solid inward, opening the supplied original face indices.
    /// At least one face must be retained. Duplicate indices are accepted once.
    /// The native API copies its source and must return a closed wall volume.
    /// This operation requires a fresh `from_brep` import with original face IDs.
    pub fn shelled_faces(&self, removed_faces: &[usize], thickness: f64) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_shell_faces(
                self.model.handle.as_ptr(),
                removed_faces.as_ptr(),
                removed_faces.len(),
                thickness,
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
