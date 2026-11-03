//! Geometry-driven polygon meshing; fixed-resolution APIs retain their layout.
use super::*;
mod grid;
mod quality;

impl NurbsSurface {
    /// Creates an editable triangle/quad mesh with normalized density controls.
    /// Curved directions refine from rational Bezier chord-error bounds;
    /// straight extrusion directions retain their smaller sampling counts.
    pub fn polygon_mesh(
        &self,
        density: Real,
        simple_planes: bool,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        let counts = self.polygon_mesh_counts(density, simple_planes, tolerance)?;
        let vertices = counts[0]
            .checked_add(1)
            .and_then(|u| counts[1].checked_add(1).and_then(|v| u.checked_mul(v)))
            .and_then(|n| n.checked_mul(self.spans_u().count()))
            .and_then(|n| n.checked_mul(self.spans_v().count()));
        if vertices.is_none_or(|n| n > 1_048_576) {
            return Err(GeometryError::MeshSubdivisionLimit);
        }
        self.tessellate_grid_with_resolution(counts, true, tolerance, false)
            .map(|(mesh, _)| mesh)
    }
    pub(crate) fn polygon_mesh_samples_per_span(
        &self,
        density: Real,
        simple_planes: bool,
        tolerance: Tolerance,
    ) -> Result<usize, GeometryError> {
        Ok(self
            .polygon_mesh_counts(density, simple_planes, tolerance)?
            .into_iter()
            .max()
            .unwrap())
    }
    fn polygon_mesh_counts(
        &self,
        density: Real,
        simple_planes: bool,
        tolerance: Tolerance,
    ) -> Result<[usize; 2], GeometryError> {
        if !density.is_finite() || !(0.0..=1.0).contains(&density) {
            return Err(GeometryError::InvalidMeshDensity(density));
        }
        if self.degree_u == 1 && self.degree_v == 1
            || simple_planes && self.plane(tolerance)?.is_some()
        {
            return Ok([1, 1]);
        }
        quality::counts(self, density)
    }
}
