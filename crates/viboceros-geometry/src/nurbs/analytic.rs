//! Owned public OpenNURBS analytic recognition shared by curve commands.
use crate::{
    Circle3, CircularArc3, CurveSegment3, GeometryError, LineSegment, NurbsCurve, Point3,
    Tolerance, Vector3,
};
use viboceros_opennurbs::AnalyticCurve;
impl NurbsCurve {
    /// Recognize a line or arc while retaining the source's native interval.
    /// This returns a representation; it does not mutate or reparameterize the
    /// input. A recognized arc may have different parameter speed.
    pub fn analytic_form(
        &self,
        tolerance: Tolerance,
    ) -> Result<Option<CurveSegment3>, GeometryError> {
        let curve = self;
        let controls = curve
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                [x, y, z, p.weight()]
            })
            .collect::<Vec<_>>();
        let result = viboceros_opennurbs::curve_analytic_form(
            curve.degree(),
            &controls,
            curve.knots(),
            tolerance.absolute(),
        )?;
        Ok(match result {
            None => None,
            Some(AnalyticCurve::Line { .. }) => Some(CurveSegment3::Line(
                LineSegment::try_new(
                    curve.evaluate(*curve.domain().start())?,
                    curve.evaluate(*curve.domain().end())?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?
                .try_reparameterized(curve.domain())?,
            )),
            Some(AnalyticCurve::Arc {
                center,
                x,
                normal,
                radius,
                angles,
            }) => {
                let circle = Circle3::try_from_frame(
                    Point3::try_from(center)?,
                    radius,
                    Vector3::try_from(x)?.normalized_nonzero()?,
                    Vector3::try_from(normal)?.normalized_nonzero()?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                Some(CurveSegment3::Arc(
                    CircularArc3::try_from_circle_angles(circle, angles[0]..=angles[1])?
                        .try_reparameterized(curve.domain())?,
                ))
            }
        })
    }
}
