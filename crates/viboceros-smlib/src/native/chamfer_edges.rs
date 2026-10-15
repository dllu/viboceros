//! Chamfers share usd-brep's selected-edge fillet executive and topology import.
use super::*;
use viboceros_geometry::Brep;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamferRailType {
    DistanceBetweenRails,
    RollingBall,
}
unsafe extern "C" {
    fn vb_imported_solid_chamfer_edges(
        solid: *mut NativeSolid,
        indices: *const usize,
        count: usize,
        distance: f64,
        accuracy: f64,
        rail_type: c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
impl Solid {
    /// Chamfer original edge indices on an owned temporary import. Sources remain unchanged.
    /// DistanceBetweenRails measures chamfer width; RollingBall measures the rolling radius.
    pub fn chamfer_brep_edges(
        brep: &Brep,
        edges: &[usize],
        distance: f64,
        rail: ChamferRailType,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        if edges.is_empty()
            || edges.len() > 100000
            || !distance.is_finite()
            || distance <= 0.
            || edges.iter().any(|&i| i >= brep.edges().len())
        {
            return Err(Error::Kernel("Invalid chamfer parameters".into()));
        }
        // Document tolerance is an upper bound; a coarse ambient import can
        // collapse the native curved-rim solve. Declared topology tolerances
        // remain intact in the shared importer.
        let working = Tolerance::try_new(
            tolerance.absolute().min(distance * 1e-9),
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let solid = Self::from_brep(brep, working)?;
        let accuracy = (distance * 1e-10).min(tolerance.absolute() * 0.1);
        call(|error, capacity| unsafe {
            vb_imported_solid_chamfer_edges(
                solid.handle.as_ptr(),
                edges.as_ptr(),
                edges.len(),
                distance,
                accuracy,
                match rail {
                    ChamferRailType::DistanceBetweenRails => 0,
                    ChamferRailType::RollingBall => 1,
                },
                error,
                capacity,
            )
        })?;
        Ok(solid)
    }
}
