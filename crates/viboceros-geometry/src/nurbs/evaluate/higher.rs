//! Fourth-order homogeneous differentiation with the shared guarded fallback.
use super::*;
use crate::nurbs::exact::{Direction, Homogeneous, Rational, curve_controls, scalar, vector};
use num_traits::Zero;
const BINOMIAL: [[u8; 5]; 5] = [
    [1, 0, 0, 0, 0],
    [1, 1, 0, 0, 0],
    [1, 2, 1, 0, 0],
    [1, 3, 3, 1, 0],
    [1, 4, 6, 4, 1],
];
impl NurbsCurve {
    /// Point and first through fourth native derivatives from a specified side.
    /// Rational quotient derivatives continue above the polynomial degree.
    pub fn evaluate_with_fourth_derivatives_on_side(
        &self,
        parameter: Real,
        side: ParameterSide,
    ) -> Result<(Point3, [Vector3; 4]), GeometryError> {
        let span = self.checked_span_on_side(parameter, side)?;
        self.with_evaluation_controls(
            span,
            |origin, net| self.fourth_float(span, parameter, origin, net),
            || self.fourth_exact(span, parameter),
        )
    }
    fn fourth_float(
        &self,
        span: usize,
        t: Real,
        origin: Point3,
        mut net: Vec<[Real; 4]>,
    ) -> Result<(Point3, [Vector3; 4]), GeometryError> {
        let mut h = [[0.; 4]; 5];
        for (order, value) in h.iter_mut().enumerate().take(5.min(self.degree + 1)) {
            *value = de_boor(
                &self.knots[order..self.knots.len() - order],
                self.degree - order,
                span - order,
                t,
                net.clone(),
            )?;
            if order < 4.min(self.degree) {
                net = self.derivative_controls(span, order + 1, &net)?;
            }
        }
        let point = project_homogeneous(h[0])?;
        let mut derivatives = [[0.; 3]; 5];
        derivatives[0] = point.to_array();
        for order in 1..=4 {
            for axis in 0..3 {
                let mut value = h[order][axis];
                let mut envelope = value.abs();
                for j in 1..=order {
                    let term =
                        Real::from(BINOMIAL[order][j]) * h[j][3] * derivatives[order - j][axis];
                    value -= term;
                    envelope += term.abs();
                }
                if !envelope.is_finite()
                    || (envelope > 0. && value.abs() <= 64. * Real::EPSILON * envelope)
                {
                    return Err(GeometryError::NonFinite {
                        context: "guarded higher NURBS quotient",
                    });
                }
                derivatives[order][axis] = value / h[0][3];
            }
        }
        let values = [
            Vector3::try_from(derivatives[1])?,
            Vector3::try_from(derivatives[2])?,
            Vector3::try_from(derivatives[3])?,
            Vector3::try_from(derivatives[4])?,
        ];
        Ok((
            self.restore_evaluated_point(span, t, point, origin)?,
            values,
        ))
    }
    fn fourth_exact(&self, span: usize, t: Real) -> Result<(Point3, [Vector3; 4]), GeometryError> {
        let mut direction = Direction {
            knots: &self.knots,
            degree: self.degree,
            span,
            parameter: t,
        };
        let mut net = curve_controls(self, span);
        let mut h: [Homogeneous; 5] =
            std::array::from_fn(|_| std::array::from_fn(|_| Rational::zero()));
        for (order, value) in h.iter_mut().enumerate().take(5.min(self.degree + 1)) {
            *value = direction.evaluate(net.clone())?;
            if order < 4.min(self.degree) {
                net = direction.derivative_controls(&net, net.len(), true)?;
                direction = direction.differentiated();
            }
        }
        if h[0][3].is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let mut d: [[Rational; 3]; 5] =
            std::array::from_fn(|_| std::array::from_fn(|_| Rational::zero()));
        d[0] = std::array::from_fn(|axis| &h[0][axis] / &h[0][3]);
        for order in 1..=4 {
            d[order] = std::array::from_fn(|axis| {
                let mut value = h[order][axis].clone();
                for j in 1..=order {
                    value -= Rational::from_integer(BINOMIAL[order][j].into())
                        * &h[j][3]
                        * &d[order - j][axis];
                }
                value / &h[0][3]
            });
        }
        let point = if let Some(p) = self.span_endpoint_point(span, t) {
            p
        } else {
            Point3::try_new(scalar(&d[0][0])?, scalar(&d[0][1])?, scalar(&d[0][2])?)?
        };
        Ok((
            point,
            [
                vector(&d[1])?,
                vector(&d[2])?,
                vector(&d[3])?,
                vector(&d[4])?,
            ],
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rational_line_fourth_jet_has_nonzero_derivatives_above_its_degree() {
        let curve = NurbsCurve::try_new_rational(
            1,
            vec![
                WeightedPoint3::try_new(Point3::try_new(0., 0., 0.).unwrap(), 1.).unwrap(),
                WeightedPoint3::try_new(Point3::try_new(1., 0., 0.).unwrap(), 2.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        for t in [0., 0.25, 0.5, 1.] {
            let (p, d) = curve
                .evaluate_with_fourth_derivatives_on_side(t, ParameterSide::Left)
                .unwrap();
            assert!((p.x() - 2. * t / (1. + t)).abs() < 1e-14);
            for (i, value) in [2., -4., 12., -48.].into_iter().enumerate() {
                assert!((d[i].x() - value / (1. + t).powi(i as i32 + 2)).abs() < 1e-12);
                assert_eq!(d[i].y(), 0.);
                assert_eq!(d[i].z(), 0.);
            }
        }
    }
    #[test]
    fn fourth_jets_share_side_limits_and_signed_weight_pole_rejection() {
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let curve = NurbsCurve::try_new(
            1,
            vec![p(0., 0.), p(1., 0.), p(1., 2.)],
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap();
        let left = curve
            .evaluate_with_fourth_derivatives_on_side(1., ParameterSide::Left)
            .unwrap();
        let right = curve
            .evaluate_with_fourth_derivatives_on_side(1., ParameterSide::Right)
            .unwrap();
        assert_eq!(left.1[0].to_array(), [1., 0., 0.]);
        assert_eq!(right.1[0].to_array(), [0., 2., 0.]);
        let singular = NurbsCurve::try_new_rational(
            1,
            vec![
                WeightedPoint3::try_new(p(0., 0.), 1.).unwrap(),
                WeightedPoint3::try_new(p(1., 0.), -1.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap();
        assert!(matches!(
            singular.evaluate_with_fourth_derivatives_on_side(0.5, ParameterSide::Right),
            Err(GeometryError::ZeroWeightAtParameter)
        ));
    }
}
