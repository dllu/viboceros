//! Public control-net conversion includes Greville UVs and evaluated normals.
use super::*;
impl NurbsSurface {
    /// Raw U/V control windows of the domain-aligned control polygon. Each
    /// periodic direction includes one closing control; other directions keep
    /// their full raw range. This does not modify the surface or its weights.
    pub fn control_polygon_ranges(&self) -> [std::ops::Range<usize>; 2] {
        let window = |degree, count, knots: &[Real], periodic| {
            let (start, end) = crate::nurbs::control_polygon_range(degree, count, knots, periodic);
            start..end
        };
        [
            window(
                self.degree_u(),
                self.control_point_count_u(),
                self.knots_u(),
                self.is_periodic_u(),
            ),
            window(
                self.degree_v(),
                self.control_point_count_v(),
                self.knots_v(),
                self.is_periodic_v(),
            ),
        ]
    }

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
