//! Native continuity classification and trim-preserving face subdivision.
use super::*;
unsafe extern "C" {
    fn vb_imported_shape_divide_creases(
        shape: *mut NativeSolid,
        kinks: c_int,
        tangents: c_int,
        tolerance: f64,
        added: *mut usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
impl Shape {
    /// Split an independently imported B-rep at native kink and/or tangent
    /// discontinuities. None preserves the original representation on a no-op.
    pub fn divide_brep_along_creases(
        brep: &viboceros_geometry::Brep,
        kinks: bool,
        tangents: bool,
        tolerance: Tolerance,
    ) -> Result<Option<(Self, usize)>, Error> {
        if !kinks && !tangents {
            return Ok(None);
        }
        let shape = Self::from_brep(brep, tolerance)?;
        let mut added = 0;
        call(|error, capacity| unsafe {
            vb_imported_shape_divide_creases(
                shape.model.handle.as_ptr(),
                kinks.into(),
                tangents.into(),
                tolerance.absolute(),
                &mut added,
                error,
                capacity,
            )
        })?;
        Ok((added > 0).then_some((shape, added)))
    }
}
