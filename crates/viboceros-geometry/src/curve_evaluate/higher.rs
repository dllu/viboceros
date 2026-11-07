//! Native fourth-order jets, preserving analytic and composite parameter charts.
use super::*;
impl CurveRef<'_> {
    /// Position and first through fourth native derivatives at a one-sided limit.
    pub fn evaluate_with_fourth_derivatives_on_side(
        self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<(Point3, [Vector3; 4]), GeometryError> {
        match self {
            Self::NurbsCurve(c) => c.evaluate_with_fourth_derivatives_on_side(parameter, side),
            Self::PolyCurve(c) => c.evaluate_with_fourth_derivatives_on_side(parameter, side),
            // Ellipses use a rational quarter chart, unlike the angular chart
            // of circles and circular arcs. Its NURBS conversion preserves it.
            Self::Ellipse(c) => c
                .to_nurbs()?
                .evaluate_with_fourth_derivatives_on_side(parameter, side),
            _ => Ok((
                self.evaluate_on_side(parameter, side)?,
                [
                    self.analytic_derivative(parameter, 1, side)?,
                    self.analytic_derivative(parameter, 2, side)?,
                    self.analytic_derivative(parameter, 3, side)?,
                    self.analytic_derivative(parameter, 4, side)?,
                ],
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle3, CircularArc3, CurveSegment3, PolyCurve3, Tolerance};
    #[test]
    fn analytic_and_composite_fourth_jets_preserve_the_native_chart() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let circle = Circle3::try_new(
            p(0., 0.),
            2.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let arc = CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2).unwrap();
        let curve = CurveRef::Arc(&arc);
        let t = curve.parameter_at(0.3).unwrap();
        let (point, d) = curve
            .evaluate_with_fourth_derivatives_on_side(t, ParameterSide::Right)
            .unwrap();
        let omega = std::f64::consts::FRAC_PI_2 / (curve.domain().end() - curve.domain().start());
        for k in 0..3 {
            assert!((d[2].to_array()[k] + omega * omega * d[0].to_array()[k]).abs() < 1e-12);
            assert!((d[3].to_array()[k] + omega * omega * d[1].to_array()[k]).abs() < 1e-12);
        }
        let poly = PolyCurve3::try_new(vec![CurveSegment3::Arc(arc)])
            .unwrap()
            .try_reparameterized(10. ..=20.)
            .unwrap();
        let (q, jet) = CurveRef::PolyCurve(&poly)
            .evaluate_with_fourth_derivatives_on_side(13., ParameterSide::Right)
            .unwrap();
        assert!(point.distance_to(q).unwrap() < 1e-12);
        let scale = (curve.domain().end() - curve.domain().start()) / 10.;
        for order in 0..4 {
            for k in 0..3 {
                assert!(
                    (jet[order].to_array()[k]
                        - d[order].to_array()[k] * scale.powi(order as i32 + 1))
                    .abs()
                        < 1e-12
                );
            }
        }
    }
}
