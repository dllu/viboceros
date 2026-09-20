//! Exact UV points and first derivatives after range loss or float failure.
use super::*;
use crate::nurbs::exact::{Direction, Homogeneous, Rational, rational, scalar};
use num_traits::Zero;

impl NurbsCurve2 {
    pub(super) fn exact_jet(
        &self,
        span: usize,
        parameter: Real,
        with_derivative: bool,
    ) -> Result<(Point2, [Real; 2]), GeometryError> {
        let direction = Direction {
            knots: &self.knots,
            degree: self.degree,
            span,
            parameter,
        };
        let controls = self.control_points[span - self.degree..=span]
            .iter()
            .map(|c| {
                let w = rational(c.weight());
                [
                    rational(c.point().x()) * &w,
                    rational(c.point().y()) * &w,
                    w,
                ]
            })
            .collect::<Vec<Homogeneous<3>>>();
        let h = direction.evaluate(controls.clone())?;
        let w = &h[2];
        if w.is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let coordinates: [Rational; 2] = std::array::from_fn(|i| &h[i] / w);
        let point = if let Some(point) = self.endpoint(span, parameter) {
            point
        } else {
            Point2::try_new(scalar(&coordinates[0])?, scalar(&coordinates[1])?)?
        };
        if !with_derivative {
            return Ok((point, [0.; 2]));
        }
        let derivatives = direction.derivative_controls(&controls, controls.len(), true)?;
        let dh = direction.differentiated().evaluate(derivatives)?;
        let derivative =
            std::array::from_fn::<_, 2, _>(|i| (&dh[i] - &coordinates[i] * &dh[2]) / w);
        Ok((point, [scalar(&derivative[0])?, scalar(&derivative[1])?]))
    }
}
