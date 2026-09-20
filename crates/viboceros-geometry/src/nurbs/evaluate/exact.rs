//! Guarded exact curve jets and scale-free limiting tangents.
use super::*;
use crate::nurbs::exact::{Direction, Homogeneous, Rational, rational, scalar, vector};
use num_traits::{Signed, Zero};

#[cfg(test)]
mod tests;

struct Evaluation<'a> {
    direction: Direction<'a>,
    controls: Vec<Homogeneous>,
    homogeneous: Homogeneous,
    coordinates: [Rational; 3],
    point: Point3,
}

impl<'a> Evaluation<'a> {
    fn new(curve: &'a NurbsCurve, span: usize, parameter: Real) -> Result<Self, GeometryError> {
        let direction = Direction {
            knots: &curve.knots,
            degree: curve.degree,
            span,
            parameter,
        };
        let controls = curve.control_points[span - curve.degree..=span]
            .iter()
            .map(|control| {
                let w = rational(control.weight());
                let p = control.point();
                [
                    rational(p.x()) * &w,
                    rational(p.y()) * &w,
                    rational(p.z()) * &w,
                    w,
                ]
            })
            .collect::<Vec<_>>();
        let homogeneous = direction.evaluate(controls.clone())?;
        if homogeneous[3].is_zero() {
            return Err(GeometryError::ZeroWeightAtParameter);
        }
        let coordinates = std::array::from_fn(|i| &homogeneous[i] / &homogeneous[3]);
        let point = if let Some(point) = curve.span_endpoint_point(span, parameter) {
            point
        } else {
            Point3::try_new(
                scalar(&coordinates[0])?,
                scalar(&coordinates[1])?,
                scalar(&coordinates[2])?,
            )?
        };
        Ok(Self {
            direction,
            controls,
            homogeneous,
            coordinates,
            point,
        })
    }
}

impl NurbsCurve {
    pub(super) fn exact_jet(
        &self,
        span: usize,
        parameter: Real,
        order: u8,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let e = Evaluation::new(self, span, parameter)?;
        let zero = Vector3::try_new(0., 0., 0.)?;
        if order == 0 {
            return Ok((e.point, zero, zero));
        }
        let d = e.direction;
        let first_controls = d.derivative_controls(&e.controls, e.controls.len(), true)?;
        let first = d.differentiated().evaluate(first_controls.clone())?;
        let w = &e.homogeneous[3];
        let derivative: [Rational; 3] =
            std::array::from_fn(|i| (&first[i] - &e.coordinates[i] * &first[3]) / w);
        let first_vector = vector(&derivative)?;
        if order == 1 {
            return Ok((e.point, first_vector, zero));
        }
        let second = if self.degree == 1 {
            std::array::from_fn(|_| Rational::zero())
        } else {
            let controls = d.differentiated().derivative_controls(
                &first_controls,
                first_controls.len(),
                true,
            )?;
            d.differentiated().differentiated().evaluate(controls)?
        };
        let second_vector = vector(&std::array::from_fn(|i| {
            (&second[i]
                - &e.coordinates[i] * &second[3]
                - &derivative[i] * &first[3]
                - &derivative[i] * &first[3])
                / w
        }))?;
        Ok((e.point, first_vector, second_vector))
    }

    pub(super) fn exact_tangent(
        &self,
        span: usize,
        parameter: Real,
        incoming: bool,
    ) -> Result<UnitVector3, GeometryError> {
        let e = Evaluation::new(self, span, parameter)?;
        let mut direction = e.direction;
        let mut controls = e.controls;
        for order in 1..=self.degree {
            controls = direction.derivative_controls(&controls, controls.len(), true)?;
            direction = direction.differentiated();
            let h = direction.evaluate(controls.clone())?;
            // Once all lower Euclidean derivatives vanish exactly, the first
            // nonzero derivative is (H^(k) - C W^(k))/W. Decide stationarity
            // before rounding, then normalize without requiring a finite speed.
            let numerator: [Rational; 3] =
                std::array::from_fn(|i| &h[i] - &e.coordinates[i] * &h[3]);
            let scale = numerator.iter().map(Signed::abs).max().unwrap();
            if !scale.is_zero() {
                let sign = if e.homogeneous[3].is_negative() {
                    -1.
                } else {
                    1.
                } * if incoming && order % 2 == 0 { -1. } else { 1. };
                return vector(&std::array::from_fn(|i| &numerator[i] / &scale))?
                    .scaled(sign)?
                    .normalized_nonzero();
            }
        }
        Err(GeometryError::Degenerate {
            context: "locally constant NURBS tangent",
        })
    }
}
