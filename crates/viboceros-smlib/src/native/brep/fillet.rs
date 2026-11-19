//! Import fresh geometry for each SMLib fillet merge.
use super::*;

impl Solid {
    /// Fillets original Rust edge indices on a freshly imported temporary solid.
    /// The input B-rep is never mutated. Repeated indices select the edge once.
    pub fn fillet_brep_edges(
        brep: &Brep,
        edges: &[usize],
        radius: f64,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        Self::fillet_brep_edges_with_corner(brep, edges, radius, tolerance, false)
    }

    /// When requested, the fillet executive requires spherical corner patches.
    pub fn fillet_brep_edges_with_corner(
        brep: &Brep,
        edges: &[usize],
        radius: f64,
        tolerance: Tolerance,
        require_ball: bool,
    ) -> Result<Self, Error> {
        Self::fillet_with_merge(brep, edges, radius, tolerance, require_ball, false)
    }

    /// Try the local topology merge on a fresh copy of the unfilleted solid.
    pub fn fillet_brep_edges_with_local_merge(
        brep: &Brep,
        edges: &[usize],
        radius: f64,
        tolerance: Tolerance,
    ) -> Result<Brep, Error> {
        Self::fillet_with_merge(brep, edges, radius, tolerance, false, true)?
            .to_brep_with_stitched_uv_joints(tolerance)
    }

    fn fillet_with_merge(
        brep: &Brep,
        edges: &[usize],
        radius: f64,
        tolerance: Tolerance,
        require_ball: bool,
        local_merge: bool,
    ) -> Result<Self, Error> {
        if edges.is_empty()
            || edges.len() > 100000
            || !radius.is_finite()
            || radius <= 0.
            || edges.iter().any(|&edge| edge >= brep.edges().len())
        {
            return Err(Error::Kernel("Invalid fillet parameters".into()));
        }
        let solid = Self::from_brep(brep, tolerance)?;
        let accuracy = (radius * 1e-10).min(tolerance.absolute() * 0.1);
        call(|error, capacity| unsafe {
            vb_imported_solid_fillet_edges(
                solid.handle.as_ptr(),
                edges.as_ptr(),
                edges.len(),
                radius,
                accuracy,
                c_int::from(require_ball),
                c_int::from(local_merge),
                error,
                capacity,
            )
        })?;
        Ok(solid)
    }
}
