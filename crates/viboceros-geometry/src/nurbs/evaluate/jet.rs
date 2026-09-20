//! Shared floating-point quotient rule and lazily prepared derivative controls.
use super::*;

type DerivativeNet = Option<Result<Vec<[Real; 4]>, GeometryError>>;

pub(super) struct FloatJet {
    pub(super) origin: Point3,
    controls: Vec<[Real; 4]>,
    first: DerivativeNet,
    second: DerivativeNet,
    scratch: Vec<[Real; 4]>,
}

impl FloatJet {
    pub(super) fn new(origin: Point3, controls: Vec<[Real; 4]>) -> Self {
        let scratch = Vec::with_capacity(controls.len());
        Self {
            origin,
            controls,
            first: None,
            second: None,
            scratch,
        }
    }

    pub(super) fn evaluate(
        &mut self,
        curve: &NurbsCurve,
        span: usize,
        parameter: Real,
        order: u8,
    ) -> Result<CurveJet, GeometryError> {
        let h = evaluate_net(curve, span, parameter, 0, &self.controls, &mut self.scratch)?;
        let point = project_homogeneous(h)?;
        let zero = Vector3::try_new(0., 0., 0.)?;
        if order == 0 {
            return Ok((
                curve.restore_evaluated_point(span, parameter, point, self.origin)?,
                zero,
                zero,
            ));
        }
        let first = self
            .first
            .get_or_insert_with(|| curve.derivative_controls(span, 1, &self.controls));
        let first = first.as_ref().map_err(Clone::clone)?;
        let h1 = evaluate_net(curve, span, parameter, 1, first, &mut self.scratch)?;
        let weight = h[3];
        let weight_derivative = h1[3];
        let coordinates = point.to_array();
        let derivative: [Real; 3] =
            std::array::from_fn(|i| (-coordinates[i]).mul_add(weight_derivative, h1[i]) / weight);
        let derivative = Vector3::try_from(derivative)?;
        if order == 1 {
            return Ok((
                curve.restore_evaluated_point(span, parameter, point, self.origin)?,
                derivative,
                zero,
            ));
        }
        let second = if curve.degree == 1 {
            // H''=0 does not imply C''=0 for unequal degree-one weights.
            let [x, y, z] = derivative.to_array().map(|value| {
                crate::parameter::scaled_ratio(value, weight_derivative, weight)
                    .map(|value| -2. * value)
            });
            Vector3::try_new(x?, y?, z?)?
        } else {
            let second = self
                .second
                .get_or_insert_with(|| curve.derivative_controls(span, 2, first));
            let second = second.as_ref().map_err(Clone::clone)?;
            let h2 = evaluate_net(curve, span, parameter, 2, second, &mut self.scratch)?;
            let first_coordinates = derivative.to_array();
            Vector3::try_from(std::array::from_fn(|i| {
                let quotient_terms =
                    (2. * weight_derivative).mul_add(first_coordinates[i], h2[3] * coordinates[i]);
                (h2[i] - quotient_terms) / weight
            }))?
        };
        Ok((
            curve.restore_evaluated_point(span, parameter, point, self.origin)?,
            derivative,
            second,
        ))
    }
}

fn evaluate_net(
    curve: &NurbsCurve,
    span: usize,
    parameter: Real,
    order: usize,
    net: &[[Real; 4]],
    scratch: &mut Vec<[Real; 4]>,
) -> Result<[Real; 4], GeometryError> {
    // Reuse capacity, never a previously evaluated station. Derivative nets
    // are shorter than the point net, which establishes maximum capacity.
    scratch.clear();
    scratch.extend_from_slice(net);
    de_boor_impl::<4, false, false>(
        &curve.knots[order..curve.knots.len() - order],
        curve.degree - order,
        span - order,
        parameter,
        scratch,
    )
}
