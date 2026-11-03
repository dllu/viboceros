//! Geometry-driven polygon meshing; fixed-resolution APIs retain their layout.
use super::*;
mod grid;
mod quality;

impl NurbsSurface {
    pub(crate) fn analysis_mesh_resolution(
        &self,
        density: crate::NurbsMeshDensity,
    ) -> Result<[usize; 2], GeometryError> {
        let mut counts = if self.degree_u == 1 && self.degree_v == 1 {
            [1, 1]
        } else {
            quality::counts(self, density.value())?
        };
        let patches = self
            .spans_u()
            .count()
            .checked_mul(self.spans_v().count())
            .ok_or(GeometryError::MeshSubdivisionLimit)?;
        while counts[0].saturating_mul(counts[1]).saturating_mul(patches) < 16 {
            let axis = usize::from(counts[1] < counts[0]);
            counts[axis] *= 2;
        }
        self.validate_mesh_resolution(counts)?;
        Ok(counts)
    }
    pub(crate) fn validate_mesh_resolution(&self, counts: [usize; 2]) -> Result<(), GeometryError> {
        let vertices = counts[0]
            .checked_add(1)
            .and_then(|u| counts[1].checked_add(1).and_then(|v| u.checked_mul(v)))
            .and_then(|n| n.checked_mul(self.spans_u().count()))
            .and_then(|n| n.checked_mul(self.spans_v().count()));
        if vertices.is_none_or(|n| n > 1_048_576) {
            Err(GeometryError::MeshSubdivisionLimit)
        } else {
            Ok(())
        }
    }
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
        self.validate_mesh_resolution(counts)?;
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
