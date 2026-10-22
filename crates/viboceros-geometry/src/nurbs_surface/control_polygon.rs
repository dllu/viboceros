//! Public control-net conversion includes Greville UVs and evaluated normals.
use super::*;
impl NurbsSurface {
    /// Native cleaned control-net mesh, including periodic seams, pole cleanup,
    /// stored normals and texture coordinates. A collapsed net has no facets.
    /// The tolerance argument is retained for API compatibility; native cleanup
    /// tests exact coincident corners rather than merging nearby controls.
    pub fn control_polygon_mesh(
        &self,
        _tolerance: Tolerance,
    ) -> Result<Option<TriangleMesh>, GeometryError> {
        let controls = self
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                [x, y, z, p.weight()]
            })
            .collect::<Vec<_>>();
        let mesh = viboceros_opennurbs::surface_control_polygon_mesh(
            viboceros_opennurbs::SurfaceDefinition {
                degree: [self.degree_u() as u32, self.degree_v() as u32],
                count: [self.control_point_count_u(), self.control_point_count_v()],
                controls: &controls,
                knots: [self.knots_u(), self.knots_v()],
            },
        )?;
        if mesh.faces.is_empty() {
            Ok(None)
        } else {
            TriangleMesh::from_open_nurbs_data(mesh).map(Some)
        }
    }
}
