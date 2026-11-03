use super::*;
impl Brep {
    pub fn curvature_mesh_with_analysis_settings(
        &self,
        settings: crate::AnalysisMeshSettings,
        tolerance: Tolerance,
    ) -> Result<crate::CurvatureMesh, GeometryError> {
        let t = self.tessellate_with_analysis_settings(settings, tolerance)?;
        self.curvature_from_tessellation(t)
    }
    pub fn curvature_mesh_with_density(
        &self,
        density: crate::NurbsMeshDensity,
        tolerance: Tolerance,
    ) -> Result<crate::CurvatureMesh, GeometryError> {
        let t = self.tessellate_with_density(density, tolerance)?;
        self.curvature_from_tessellation(t)
    }
    /// Curvature samples share the tessellator's translated face charts and
    /// one-sided crease choices, avoiding nearest-point projection.
    pub fn curvature_mesh(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<crate::CurvatureMesh, GeometryError> {
        let t = self.tessellate_with_normals(samples_per_span, tolerance)?;
        self.curvature_from_tessellation(t)
    }
    fn curvature_from_tessellation(
        &self,
        t: crate::SurfaceTessellation,
    ) -> Result<crate::CurvatureMesh, GeometryError> {
        let frames = self
            .faces
            .iter()
            .map(BrepFace::local_parameter_frame)
            .collect::<Result<Vec<_>, _>>()?;
        let surfaces = frames.iter().map(|f| &f.face.surface).collect::<Vec<_>>();
        Ok(crate::CurvatureMesh::from_tessellation(t, &surfaces))
    }
}
