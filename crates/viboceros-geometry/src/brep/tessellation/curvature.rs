use super::*;
impl Brep {
    /// Curvature samples share the tessellator's translated face charts and
    /// one-sided crease choices, avoiding nearest-point projection.
    pub fn curvature_mesh(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<crate::CurvatureMesh, GeometryError> {
        let t = self.tessellate_with_normals(samples_per_span, tolerance)?;
        let frames = self
            .faces
            .iter()
            .map(BrepFace::local_parameter_frame)
            .collect::<Result<Vec<_>, _>>()?;
        let surfaces = frames.iter().map(|f| &f.face.surface).collect::<Vec<_>>();
        Ok(crate::CurvatureMesh::from_tessellation(t, &surfaces))
    }
}
