use super::*;
use viboceros_geometry::Brep;

unsafe extern "C" {
    fn vb_shape_bounds(
        shape: *const NativeSolid,
        bounds: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_face_count(
        shape: *const NativeSolid,
        count: *mut usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_shape_stitch(
        shape: *const NativeSolid,
        distance: f64,
        face_origins: *mut usize,
        face_capacity: usize,
        face_count: *mut usize,
        stitched_edges: *mut usize,
        gaps: *mut f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
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

/// Copied-operand stitching result with original face indices and native gaps.
pub struct StitchResult {
    pub shape: Shape,
    /// Input face index for each face in the exported output order.
    pub face_origins: Vec<usize>,
    pub stitched_edges: usize,
    /// Largest candidate gaps considered by SMLib, including rejected pairs.
    /// These are diagnostics, not bounds on the final boundary deviation.
    pub maximum_vertex_gap: f64,
    pub maximum_edge_gap: f64,
}

impl StitchResult {
    /// Restore source UV axes when native normal unification transposes an
    /// otherwise identical NURBS surface. The face sense and trims compensate
    /// the chart change, preserving the native oriented spatial geometry.
    /// Normalized, clamped or otherwise different control nets remain native;
    /// no tolerance-based coefficient matching is used.
    pub fn to_brep_preserving_source_uv(
        &self,
        source: &Brep,
        tolerance: Tolerance,
    ) -> Result<Brep, Error> {
        let mut output = self.shape.to_brep(tolerance)?;
        if output.faces().len() != self.face_origins.len()
            || source.faces().len() != self.face_origins.len()
        {
            return Err(Error::InvalidOutput);
        }
        let mut swapped = Vec::new();
        for (index, (face, &origin)) in output.faces().iter().zip(&self.face_origins).enumerate() {
            let original = source.faces().get(origin).ok_or(Error::InvalidOutput)?;
            if face.surface() != original.surface()
                && face.surface().try_swapped_uv()? == *original.surface()
            {
                swapped.push(index);
            }
        }
        output.try_swap_face_uv(&swapped)?;
        Ok(output)
    }
}

impl Shape {
    /// Query the native trimmed boundary's tight bounding box, including sheets.
    pub fn tight_bounds(&self) -> Result<viboceros_geometry::BoundingBox3, Error> {
        let mut values = [0.; 6];
        call(|error, capacity| unsafe {
            vb_shape_bounds(
                self.model.handle.as_ptr(),
                values.as_mut_ptr(),
                error,
                capacity,
            )
        })?;
        Ok(viboceros_geometry::BoundingBox3::from_points([
            Point3::try_new(values[0], values[1], values[2])?,
            Point3::try_new(values[3], values[4], values[5])?,
        ])?)
    }
    /// Stitch a private copy using SMLib's manifold edge-pair mode.
    /// The effective radius is the larger of `distance` and the sum of each
    /// candidate pair's stored tolerances. Individual glue failures are skipped;
    /// remaining open boundaries are retained. Source geometry is preserved.
    pub fn stitch(&self, distance: f64) -> Result<StitchResult, Error> {
        if !distance.is_finite() || distance <= 0. {
            return Err(Error::Kernel(
                "Stitch distance must be finite and positive".into(),
            ));
        }
        let mut face_capacity = 0;
        call(|error, capacity| unsafe {
            vb_shape_face_count(
                self.model.handle.as_ptr(),
                &mut face_capacity,
                error,
                capacity,
            )
        })?;
        if face_capacity == 0 || face_capacity > 1_000_000 {
            return Err(Error::InvalidOutput);
        }
        let mut face_origins = vec![usize::MAX; face_capacity];
        let (mut face_count, mut stitched_edges) = (0, 0);
        let mut gaps = [0.; 2];
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_stitch(
                self.model.handle.as_ptr(),
                distance,
                face_origins.as_mut_ptr(),
                face_capacity,
                &mut face_count,
                &mut stitched_edges,
                gaps.as_mut_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        let shape = Shape {
            model: Solid {
                handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
                _thread_local: PhantomData,
            },
        };
        if face_count != face_capacity
            || face_origins[..face_count]
                .iter()
                .any(|&i| i >= face_capacity)
        {
            return Err(Error::InvalidOutput);
        }
        let mut seen = vec![false; face_count];
        for &origin in &face_origins {
            if std::mem::replace(&mut seen[origin], true) {
                return Err(Error::InvalidOutput);
            }
        }
        face_origins.truncate(face_count);
        Ok(StitchResult {
            shape,
            face_origins,
            stitched_edges,
            maximum_vertex_gap: gaps[0],
            maximum_edge_gap: gaps[1],
        })
    }
    /// Import manifold sheets or closed solids through the shared NURBS payload.
    /// Mixed open/closed components and nonmanifold sheets are rejected.
    pub fn from_brep(brep: &Brep, tolerance: Tolerance) -> Result<Self, Error> {
        Ok(Self {
            model: if brep.is_solid() {
                Solid::from_brep(brep, tolerance)?
            } else {
                brep::import_sheet(brep, tolerance, false)?
            },
        })
    }

    /// Import open manifold sheets with unit edge and surface parameter domains.
    /// Spatial geometry and tolerances are preserved. UV trim coordinates are
    /// mapped with their surface, and trim parameters with their spatial edge.
    /// Use after spatial normalization when extreme knot intervals would exceed
    /// the native solvers' numerical range. Source geometry is unchanged.
    pub fn from_sheet_with_normalized_domains(
        brep: &Brep,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        Ok(Self {
            model: brep::import_sheet(brep, tolerance, true)?,
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
