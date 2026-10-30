//! Planar control-boundary caps appended to the original native record.
use super::*;
use crate::Tolerance;
mod regions;
#[cfg(test)]
mod tests;

impl SubD {
    /// Caps planar control-boundary regions. Convex loops receive a single
    /// face or a bounding-box-center triangle fan; concave/holed regions use
    /// constrained triangles. Creased joins retain the cap boundary, while
    /// relaxed joins are smoothed by the public OpenNURBS tag setters.
    /// Original components and face properties survive; closed inward results
    /// are normalized through the public solid-orientation query.
    pub fn cap_planar_holes_with_options(
        &self,
        tolerance: Tolerance,
        crease: bool,
        triangles: bool,
    ) -> Result<(Self, usize), GeometryError> {
        let draft = regions::prepare(self, tolerance, triangles)?;
        if draft.faces.is_empty() {
            return Ok((self.clone(), 0));
        }
        let points = draft
            .points
            .iter()
            .map(|p| p.to_array())
            .collect::<Vec<_>>();
        Self::from_native(viboceros_opennurbs::subd_append_caps(
            self.archive(),
            &points,
            &draft.faces,
            &draft.boundary_edges,
            crease,
        )?)
        .map(|capped| (capped, draft.boundaries))
    }
}
