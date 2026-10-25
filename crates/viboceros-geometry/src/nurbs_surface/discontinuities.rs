//! Public OpenNURBS isocurve classification without building sampled curves.
use super::*;

impl NurbsSurface {
    /// Strictly interior discontinuities of the surface isocurves in U and V.
    /// Uses public OpenNURBS G1, G2, or Gsmooth classification with an angle in
    /// radians. Singular transverse endpoints are omitted before traversal;
    /// closed seams and the source tensor are retained. The native predicate
    /// samples transverse spans, not a continuous maximum-angle certificate.
    pub fn discontinuity_parameters(
        &self,
        continuity: crate::CurveContinuity,
        angle: Real,
    ) -> Result<[Vec<Real>; 2], GeometryError> {
        if !angle.is_finite() || !(0.0..=std::f64::consts::PI).contains(&angle) {
            return Err(GeometryError::InvalidContinuityAngle);
        }
        let controls = self
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                [x, y, z, p.weight()]
            })
            .collect::<Vec<_>>();
        Ok(viboceros_opennurbs::surface_continuity_breaks(
            viboceros_opennurbs::SurfaceDefinition {
                degree: [self.degree_u() as u32, self.degree_v() as u32],
                count: [self.control_point_count_u(), self.control_point_count_v()],
                controls: &controls,
                knots: [self.knots_u(), self.knots_v()],
            },
            [true; 2],
            continuity,
            angle.cos(),
        )?)
    }
}
