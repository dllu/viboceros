//! Query-owned last-span coefficients; no global caches or station memoization.
use super::*;
use exact::ExactJetNet;

#[cfg(test)]
mod tests;

enum Prepared<'a> {
    Float(FloatJet),
    Exact(ExactJetNet<'a>),
}

pub(in crate::nurbs) struct CurveQuery<'a> {
    curve: &'a NurbsCurve,
    active: Option<(usize, Prepared<'a>)>,
}

impl<'a> CurveQuery<'a> {
    pub(in crate::nurbs) fn new(curve: &'a NurbsCurve) -> Self {
        Self {
            curve,
            active: None,
        }
    }

    pub(in crate::nurbs) fn evaluate(&mut self, parameter: Real) -> Result<Point3, GeometryError> {
        Ok(self.jet(parameter, ParameterSide::Right, 0)?.0)
    }

    pub(in crate::nurbs) fn evaluate_with_derivative(
        &mut self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let (p, d, _) = self.jet(parameter, ParameterSide::Right, 1)?;
        Ok((p, d))
    }

    pub(in crate::nurbs) fn evaluate_with_second_derivative(
        &mut self,
        parameter: Real,
    ) -> Result<CurveJet, GeometryError> {
        self.jet(parameter, ParameterSide::Right, 2)
    }

    pub(super) fn jet(
        &mut self,
        parameter: Real,
        side: ParameterSide,
        order: u8,
    ) -> Result<CurveJet, GeometryError> {
        let curve = self.curve;
        let span = curve.checked_span_on_side(parameter, side)?;
        // Preserve the public point evaluator's exact endpoint shortcut. Jets
        // still validate/project the homogeneous station before derivatives.
        if order == 0
            && let Some(point) = curve.span_endpoint_point(span, parameter)
        {
            let zero = Vector3::try_new(0., 0., 0.)?;
            return Ok((point, zero, zero));
        }
        if self
            .active
            .as_ref()
            .is_none_or(|(previous, _)| *previous != span)
        {
            let controls = curve.homogeneous_controls(span, true)?;
            let prepared = if controls.needs_exact {
                Prepared::Exact(ExactJetNet::new(curve, span))
            } else {
                Prepared::Float(FloatJet::new(controls.origin, controls.homogeneous))
            };
            self.active = Some((span, prepared));
        }
        match &mut self
            .active
            .as_mut()
            .expect("active curve span initialized")
            .1
        {
            Prepared::Exact(net) => net.evaluate(parameter, order),
            Prepared::Float(net) => {
                let result = net.evaluate(curve, span, parameter, order);
                // Match public recovery precedence. A late failure does not
                // permanently change this span's floating-point dispatch.
                if matches!(result, Err(GeometryError::NonFinite { .. }))
                    && net.origin.to_array() != [0.; 3]
                {
                    let unshifted = curve.homogeneous_controls(span, false)?;
                    if unshifted.needs_exact {
                        curve.exact_jet(span, parameter, order)
                    } else {
                        FloatJet::new(unshifted.origin, unshifted.homogeneous)
                            .evaluate(curve, span, parameter, order)
                            .or_else(|_| curve.exact_jet(span, parameter, order))
                    }
                } else {
                    result.or_else(|_| curve.exact_jet(span, parameter, order))
                }
            }
        }
    }
}
