//! Parameter correspondence with the exact rational representation.

use crate::circular::circular_nurbs_span_count;
use crate::parameter::{checked_parameter, map_parameter};
use crate::{CurveEvaluationSide, CurveRef, GeometryError, Real};
use std::f64::consts::TAU;

#[cfg(test)]
mod tests;

impl CurveRef<'_> {
    /// Converts a native parameter to the corresponding parameter of
    /// [`Self::to_nurbs`]. This is not generally an identity for circular curves.
    /// The map follows the original spans, so coincident points on different
    /// branches retain distinct parameters. No closest-point search is used.
    pub fn nurbs_parameter(self, parameter: Real) -> Result<Real, GeometryError> {
        self.map_nurbs_parameter(parameter, true)
    }

    /// Converts a parameter of [`Self::to_nurbs`] back to the native curve.
    /// Both representations have the same domain and exactly matching endpoints.
    /// This correspondence applies to the unedited rational representation,
    /// including affine transformations that preserve its parameterization.
    pub fn parameter_from_nurbs(self, parameter: Real) -> Result<Real, GeometryError> {
        self.map_nurbs_parameter(parameter, false)
    }

    fn map_nurbs_parameter(self, parameter: Real, to_nurbs: bool) -> Result<Real, GeometryError> {
        let domain = self.domain();
        checked_parameter(parameter, domain.clone())?;
        if parameter == *domain.start() || parameter == *domain.end() {
            return Ok(parameter);
        }
        let sweep = match self {
            Self::Circle(_) => TAU,
            Self::Arc(arc) => arc.sweep_radians(),
            Self::PolyCurve(curve) => {
                let index = curve.segment_index(parameter, CurveEvaluationSide::Right)?;
                let segment = curve.segments()[index].as_ref();
                // Non-circular leaves are already parameter equivalent. Avoid
                // rounding a parameter through two unnecessary affine maps.
                if !matches!(segment, Self::Arc(_)) {
                    return Ok(parameter);
                }
                let local = curve.segment_parameter(index, parameter)?;
                return curve
                    .polycurve_parameter(index, segment.map_nurbs_parameter(local, to_nurbs)?);
            }
            // Ellipse3 uses rational-quarter parameters already, unlike Circle3.
            _ => return Ok(parameter),
        };
        let spans = circular_nurbs_span_count(sweep);
        let mut span = spans - 1;
        for boundary in 1..spans {
            if parameter < self.parameter_at(boundary as Real / spans as Real)? {
                span = boundary - 1;
                break;
            }
        }
        // Map within the original span interval, retaining exact quarter knots
        // and accuracy near the endpoints of a translated parameter domain.
        let span_start = self.parameter_at(span as Real / spans as Real)?;
        let span_end = self.parameter_at((span + 1) as Real / spans as Real)?;
        if parameter == span_start || parameter == span_end {
            return Ok(parameter);
        }
        let fraction = map_parameter(parameter, span_start..=span_end, 0.0..=1.0)?;
        let half = sweep / spans as Real * 0.5;
        if half < Real::EPSILON.sqrt() {
            // The nonlinear correction is below floating-point precision.
            // This also avoids underflow in sin(half) for microscopic sweeps.
            return Ok(parameter);
        }
        let mapped = if to_nurbs {
            let a = (half * fraction).sin();
            let b = (half * (1.0 - fraction)).sin();
            a / (a + b)
        } else {
            // tan(theta/2) = q sin(half) / (1-q + q cos(half)).
            (fraction * half.sin()).atan2((1.0 - fraction) + fraction * half.cos()) / half
        };
        map_parameter(mapped.clamp(0.0, 1.0), 0.0..=1.0, span_start..=span_end)
    }
}
