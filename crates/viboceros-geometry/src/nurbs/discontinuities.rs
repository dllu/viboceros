//! Parametric and geometric continuity from public OpenNURBS queries.
use super::*;

impl NurbsCurve {
    /// Strictly interior C0, C1 or C2 breaks using public OpenNURBS tests.
    /// C1 and C2 compare derivatives in the source chart, including speed;
    /// returned parameters retain that chart. The source and closed seams are
    /// unchanged. Uses the adapter's existing bounded traversal and buffers.
    pub fn parametric_discontinuity_parameters(
        &self,
        continuity: crate::CurveParametricContinuity,
    ) -> Result<Vec<Real>, GeometryError> {
        let controls = native_controls(self);
        Ok(viboceros_opennurbs::curve_parametric_continuity_breaks(
            self.degree(),
            &controls,
            self.knots(),
            continuity,
        )?)
    }

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
        let controls = native_controls(self);
        Ok(viboceros_opennurbs::curve_continuity_breaks(
            self.degree(),
            &controls,
            self.knots(),
            continuity,
            angle.cos(),
        )?)
    }
}

fn native_controls(curve: &NurbsCurve) -> Vec<[Real; 4]> {
    curve
        .control_points()
        .iter()
        .map(|p| {
            let [x, y, z] = p.point().to_array();
            [x, y, z, p.weight()]
        })
        .collect()
}
