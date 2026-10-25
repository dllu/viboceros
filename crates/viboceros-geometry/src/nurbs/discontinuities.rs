//! Geometric continuity from the public OpenNURBS query.
use super::*;

impl NurbsCurve {
    /// Strictly interior geometric continuity breaks. The angle is in radians.
    /// Parameter speed changes alone do not
    /// introduce cuts. Closed seams and the source representation are retained.
    pub fn discontinuity_parameters(
        &self,
        continuity: viboceros_opennurbs::CurveContinuity,
        angle: Real,
    ) -> Result<Vec<Real>, GeometryError> {
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
        Ok(viboceros_opennurbs::curve_continuity_breaks(
            self.degree(),
            &controls,
            self.knots(),
            continuity,
            angle.cos(),
        )?)
    }
}
