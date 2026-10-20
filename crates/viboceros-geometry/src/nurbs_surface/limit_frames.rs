//! Public OpenNURBS surface frames include singular boundary limit handling.
use super::*;
impl NurbsSurface {
    /// Evaluates a batch of public OpenNURBS parameter frames on an owned exact
    /// definition. Unlike `frame_at`, this includes boundary limit frames where
    /// a first derivative vanishes. A missing native frame remains explicit.
    pub fn limit_frames_at(
        &self,
        parameters: &[[Real; 2]],
    ) -> Result<Vec<Option<Frame3>>, GeometryError> {
        let controls = self
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                [x, y, z, p.weight()]
            })
            .collect::<Vec<_>>();
        let frames = viboceros_opennurbs::surface_frames(
            viboceros_opennurbs::SurfaceDefinition {
                degree: [self.degree_u() as u32, self.degree_v() as u32],
                count: [self.control_point_count_u(), self.control_point_count_v()],
                controls: &controls,
                knots: [self.knots_u(), self.knots_v()],
            },
            parameters,
        )?;
        frames
            .into_iter()
            .map(|f| {
                f.map(|f| {
                    Frame3::try_from_directions(
                        Point3::try_from([f[0], f[1], f[2]])?,
                        Vector3::try_from([f[3], f[4], f[5]])?,
                        Vector3::try_from([f[6], f[7], f[8]])?,
                        Tolerance::NUMERICAL_VALIDATION,
                    )
                })
                .transpose()
            })
            .collect()
    }
}
