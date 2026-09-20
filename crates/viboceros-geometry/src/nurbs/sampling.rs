//! Parameter-fraction sampling without first restoring a large knot origin.

use super::*;
use crate::{ParameterSide, parameter::lossless_parameter_origin};
use std::borrow::Cow;

/// Reusable equal-parameter (not equal-arc-length) sampling frame.
///
/// A same-sign domain is translated toward zero only if every knot, including
/// exterior knots, shifts exactly. Controls and weights are unchanged. Curves
/// needing no shift, or whose knots cannot all shift exactly, are borrowed.
/// No knots are scaled, fitted, or collapsed. Arbitrarily ill-conditioned
/// relative span widths and subnormal fractional parameters remain subject to
/// floating-point resolution; this frame is not an arbitrary-precision sampler.
pub struct NurbsCurveParameterSampler<'a> {
    curve: Cow<'a, NurbsCurve>,
}

/// One nonempty span in a [`NurbsCurveParameterSampler`]. Its endpoints use
/// the right and left limits respectively, even at discontinuous knots.
#[derive(Clone, Copy)]
pub struct NurbsCurveSamplingSpan<'a> {
    curve: &'a NurbsCurve,
    start: Real,
    end: Real,
}

impl NurbsCurve {
    /// Prepares a frame for repeated fractional point sampling without
    /// modifying this curve's stored domain or control net.
    ///
    /// Unlike `evaluate(parameter_at(fraction)?)`, internal stations do not
    /// first round back onto a large native knot origin. Point evaluation
    /// retains the ordinary evaluator's guarded exact-arithmetic fallback.
    pub fn parameter_sampler(&self) -> Result<NurbsCurveParameterSampler<'_>, GeometryError> {
        let origin = lossless_parameter_origin(self.domain(), self.knots.iter().copied());
        let curve = if origin == 0. {
            Cow::Borrowed(self)
        } else {
            Cow::Owned(Self::try_new_rational(
                self.degree,
                self.control_points.clone(),
                self.knots.iter().map(|k| k - origin).collect(),
            )?)
        };
        Ok(NurbsCurveParameterSampler { curve })
    }
}

impl NurbsCurveParameterSampler<'_> {
    /// Evaluates a fraction in `[0, 1]` of the entire active domain, using the
    /// right side at interior knots. No native parameter is returned.
    pub fn evaluate(&self, fraction: Real) -> Result<Point3, GeometryError> {
        let domain = self.curve.domain();
        self.curve
            .evaluate(sample_parameter(*domain.start(), *domain.end(), fraction)?)
    }

    /// Visits nonempty spans in parameter order. Sample each span separately
    /// to avoid connecting across discontinuities.
    pub fn spans(&self) -> impl Iterator<Item = NurbsCurveSamplingSpan<'_>> {
        self.curve
            .spans()
            .map(|(start, end)| NurbsCurveSamplingSpan {
                curve: &self.curve,
                start,
                end,
            })
    }
}

impl NurbsCurveSamplingSpan<'_> {
    /// Evaluates a fraction in `[0, 1]` of this span. The end uses its exact
    /// incoming limit, not a neighboring floating-point parameter. A rounded
    /// interior station also stays on this span's side of either boundary.
    pub fn evaluate(self, fraction: Real) -> Result<Point3, GeometryError> {
        let parameter = sample_parameter(self.start, self.end, fraction)?;
        let side = if parameter == self.end {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        };
        self.curve.evaluate_on_side(parameter, side)
    }
}

fn sample_parameter(start: Real, end: Real, fraction: Real) -> Result<Real, GeometryError> {
    crate::parameter::checked_parameter(fraction, 0.0..=1.0)?;
    // Keep natural endpoints exact, including when the interval width would
    // overflow. Clamp a rounded convex combination to the requested span.
    Ok(if fraction == 0. {
        start
    } else if fraction == 1. {
        end
    } else {
        start
            .mul_add(1. - fraction, end * fraction)
            .clamp(start, end)
    })
}

#[cfg(test)]
mod tests;
