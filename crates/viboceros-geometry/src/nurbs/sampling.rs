//! Parameter-fraction sampling without first restoring a large knot origin.

use super::*;
use crate::{ParameterSide, parameter::lossless_parameter_origin};
use std::borrow::Cow;
mod exact;

/// Reusable equal-parameter (not equal-arc-length) sampling frame.
///
/// A same-sign domain is translated toward zero only if every knot, including
/// exterior knots, shifts exactly. Controls and weights are unchanged. Curves
/// needing no shift, or whose knots cannot all shift exactly, are borrowed.
/// No knots are scaled, fitted, or collapsed. Subnormal intervals/fractions,
/// adjacent-float spans, and declined origin shifts use exact fractional
/// parameters and homogeneous evaluation. Intermediate range loss, failed
/// evaluations, and whole-domain positional jumps use the same recovery.
/// Other samples retain the ordinary
/// floating-point evaluator; this is not universally correctly rounded sampling.
pub struct NurbsCurveParameterSampler<'a> {
    curve: Cow<'a, NurbsCurve>,
    origin_declined: bool,
    exact_domain: bool,
}

/// One nonempty span in a [`NurbsCurveParameterSampler`]. Its endpoints use
/// the right and left limits respectively, even at discontinuous knots.
#[derive(Clone, Copy)]
pub struct NurbsCurveSamplingSpan<'a> {
    curve: &'a NurbsCurve,
    start: Real,
    end: Real,
    span: usize,
    exact_interval: bool,
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
        let origin_declined =
            origin == 0. && (*self.domain().start() > 0. || *self.domain().end() < 0.);
        let curve = if origin == 0. {
            Cow::Borrowed(self)
        } else {
            Cow::Owned(Self::try_new_rational(
                self.degree,
                self.control_points.clone(),
                self.knots.iter().map(|k| k - origin).collect(),
            )?)
        };
        // A rounded whole-domain station must not choose the wrong branch
        // at a full-order interior knot. Ordinary C0/C1 joins stay fast.
        let discontinuous = curve.knots[curve.degree..=curve.control_points.len()]
            .windows(curve.degree + 1)
            .any(|knots| knots[0] == knots[curve.degree]);
        let exact_domain =
            origin_declined || discontinuous || curve.spans().any(|(a, b)| exact_interval(a, b));
        Ok(NurbsCurveParameterSampler {
            curve,
            origin_declined,
            exact_domain,
        })
    }
}

impl NurbsCurveParameterSampler<'_> {
    /// Evaluates a fraction in `[0, 1]` of the entire active domain, using the
    /// right side at interior knots. No native parameter is returned.
    pub fn evaluate(&self, fraction: Real) -> Result<Point3, GeometryError> {
        let domain = self.curve.domain();
        let start = *domain.start();
        let end = *domain.end();
        let parameter = sample_parameter(start, end, fraction)?;
        let interior = fraction > 0. && fraction < 1.;
        if interior && (self.exact_domain || lost_station_range(start, end, fraction, parameter)) {
            return exact::point(&self.curve, start, end, fraction, None);
        }
        self.curve.evaluate(parameter).or_else(|error| {
            if interior {
                exact::point(&self.curve, start, end, fraction, None)
            } else {
                Err(error)
            }
        })
    }

    /// Visits nonempty spans in parameter order. Sample each span separately
    /// to avoid connecting across discontinuities.
    pub fn spans(&self) -> impl Iterator<Item = NurbsCurveSamplingSpan<'_>> {
        self.curve
            .knots
            .windows(2)
            .enumerate()
            .skip(self.curve.degree)
            .take(self.curve.control_points.len() - self.curve.degree)
            .filter(|(_, pair)| pair[0] < pair[1])
            .map(|(span, pair)| NurbsCurveSamplingSpan {
                curve: &self.curve,
                start: pair[0],
                end: pair[1],
                span,
                exact_interval: self.origin_declined || exact_interval(pair[0], pair[1]),
            })
    }
}

impl NurbsCurveSamplingSpan<'_> {
    /// Evaluates a fraction in `[0, 1]` of this span. The end uses its exact
    /// incoming limit, not a neighboring floating-point parameter. A rounded
    /// interior station also stays on this span's side of either boundary.
    #[inline]
    pub fn evaluate(self, fraction: Real) -> Result<Point3, GeometryError> {
        let parameter = sample_parameter(self.start, self.end, fraction)?;
        let interior = fraction > 0. && fraction < 1.;
        if interior
            && (self.exact_interval
                || lost_station_range(self.start, self.end, fraction, parameter))
        {
            return exact::point(self.curve, self.start, self.end, fraction, Some(self.span));
        }
        let side = if parameter == self.end {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        };
        self.curve
            .evaluate_on_side(parameter, side)
            .or_else(|error| {
                if interior {
                    exact::point(self.curve, self.start, self.end, fraction, Some(self.span))
                } else {
                    Err(error)
                }
            })
    }
}

fn exact_interval(start: Real, end: Real) -> bool {
    (end - start).is_subnormal() || start.next_up() == end
}

fn lost_station_range(start: Real, end: Real, fraction: Real, parameter: Real) -> bool {
    fraction.is_subnormal() || parameter.is_subnormal() || parameter == start || parameter == end
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
mod reference_tests;
#[cfg(test)]
mod tests;
