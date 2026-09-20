//! Guarded exact curve jets and scale-free limiting tangents.
use super::*;
use crate::nurbs::exact::{Direction, Homogeneous, Rational, curve_controls, scalar, vector};
use num_traits::{Signed, Zero};

#[cfg(test)]
mod tests;

struct Evaluation<'a> {
    direction: Direction<'a>,
    homogeneous: Homogeneous,
    coordinates: [Rational; 3],
    point: Point3,
}

impl<'a> Evaluation<'a> {
    fn new(
        curve: &'a NurbsCurve,
        span: usize,
        parameter: Real,
        controls: &[Homogeneous],
    ) -> Result<Self, GeometryError> {
        let direction = Direction {
            knots: &curve.knots,
            degree: curve.degree,
            span,
            parameter,
        };
        let homogeneous = direction.evaluate(controls.to_vec())?;
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
            homogeneous,
            coordinates,
            point,
        })
    }
}

pub(super) struct ExactJetNet<'a> {
    curve: &'a NurbsCurve,
    span: usize,
    controls: Vec<Homogeneous>,
    first: Option<Vec<Homogeneous>>,
    second: Option<Vec<Homogeneous>>,
}

impl<'a> ExactJetNet<'a> {
    pub(super) fn new(curve: &'a NurbsCurve, span: usize) -> Self {
        Self {
            curve,
            span,
            controls: curve_controls(curve, span),
            first: None,
            second: None,
        }
    }

    pub(super) fn evaluate(
        &mut self,
        parameter: Real,
        order: u8,
    ) -> Result<CurveJet, GeometryError> {
        // Re-evaluate the denominator before using any derivative net. These
        // caches contain coefficients, never a previous station's pole status.
        let e = Evaluation::new(self.curve, self.span, parameter, &self.controls)?;
        let zero = Vector3::try_new(0., 0., 0.)?;
        if order == 0 {
            return Ok((e.point, zero, zero));
        }
        let d = e.direction;
        if self.first.is_none() {
            self.first = Some(d.derivative_controls(&self.controls, self.controls.len(), true)?);
        }
        let first_controls = self
            .first
            .as_ref()
            .expect("first exact curve net initialized");
        let first = d.differentiated().evaluate(first_controls.clone())?;
        let w = &e.homogeneous[3];
        let derivative: [Rational; 3] =
            std::array::from_fn(|i| (&first[i] - &e.coordinates[i] * &first[3]) / w);
        let first_vector = vector(&derivative)?;
        if order == 1 {
            return Ok((e.point, first_vector, zero));
        }
        let second = if self.curve.degree == 1 {
            std::array::from_fn(|_| Rational::zero())
        } else {
            if self.second.is_none() {
                self.second = Some(d.differentiated().derivative_controls(
                    first_controls,
                    first_controls.len(),
                    true,
                )?);
            }
            d.differentiated().differentiated().evaluate(
                self.second
                    .as_ref()
                    .expect("second exact curve net initialized")
                    .clone(),
            )?
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
}

impl NurbsCurve {
    pub(super) fn exact_jet(
        &self,
        span: usize,
        parameter: Real,
        order: u8,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        ExactJetNet::new(self, span).evaluate(parameter, order)
    }

    pub(super) fn exact_tangent(
        &self,
        span: usize,
        parameter: Real,
        incoming: bool,
    ) -> Result<UnitVector3, GeometryError> {
        let controls = curve_controls(self, span);
        let e = Evaluation::new(self, span, parameter, &controls)?;
        let mut direction = e.direction;
        let mut controls = controls;
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
