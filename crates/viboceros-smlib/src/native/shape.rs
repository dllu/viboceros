use super::*;
use viboceros_geometry::Brep;

unsafe extern "C" {
    fn vb_shape_offset(
        shape: *const NativeSolid,
        distance: f64,
        wall: c_int,
        accuracy: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

impl Shape {
    /// Import manifold sheets or closed solids through the shared NURBS payload.
    /// Mixed open/closed components and nonmanifold sheets are rejected.
    pub fn from_brep(brep: &Brep, tolerance: Tolerance) -> Result<Self, Error> {
        Ok(Self {
            model: if brep.is_solid() {
                Solid::from_brep(brep, tolerance)?
            } else {
                brep::import_sheet(brep, tolerance)?
            },
        })
    }

    /// Create an independent normal offset or wall volume using the kernel.
    pub fn offset(&self, distance: f64, solid: bool) -> Result<Self, Error> {
        self.offset_with_accuracy(distance, solid, 1e-9)
    }

    /// Set creation accuracy explicitly; the shared native tolerance is restored
    /// before returning on success or failure.
    pub fn offset_with_accuracy(
        &self,
        distance: f64,
        solid: bool,
        accuracy: f64,
    ) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_offset(
                self.model.handle.as_ptr(),
                distance,
                c_int::from(solid),
                accuracy,
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

    /// Transfer exact owned NURBS geometry and manifold topology.
    pub fn to_brep(&self, tolerance: Tolerance) -> Result<Brep, Error> {
        self.model.to_brep(tolerance)
    }

    /// Transfer an offset with explicit native NURBS approximation tolerance.
    /// Kernel approximation failures or an exceeded achieved bound propagate.
    pub fn to_brep_approximated(
        &self,
        tolerance: Tolerance,
        approximation: f64,
    ) -> Result<Brep, Error> {
        self.model
            .to_brep_with_approximation(tolerance, Some(approximation))
    }
}
