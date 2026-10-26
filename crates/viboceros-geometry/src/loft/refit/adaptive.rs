//! Bounded adaptive proposals for sources with certified straight regions.
use super::*;
use crate::{ArcLengthSampler, CurveRef, ParameterSide, Point3, Tolerance};

/// Certified C0 cubic pieces before native Loft joining and region matching.
/// This does not establish Rhino's final control net or parameter chart.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveLinearRefitProposal {
    pub curve: NurbsCurve,
    /// Normalized arc-length intervals used to construct the raw pieces.
    pub arc_length_breaks: Vec<Real>,
    /// Complete symmetric image-distance bound against the original source.
    pub maximum_deviation_bound: Real,
}

const MAX_PIECES: usize = 128;
const MAX_DEPTH: usize = 24;

/// Propose bounded adaptive cubics for exactly extracted straight regions with
/// no resolved one-degree tangent breaks. The source image and limiting frames
/// are retained through an exact, uniformly parameterized polygon proxy.
/// Seven interior stations select pieces using half the requested tolerance;
/// acceptance requires a complete image bound within the requested tolerance.
/// This raw result retains C0 joins. The command path does not use these
/// proposals while adaptive acceptance remains unqualified. Limits are 128 pieces,
/// depth 24, 512 source controls, and the shared exact-arithmetic work limits.
/// `None` means unsupported or unproved; no source mutation or partial output
/// escapes failure.
pub fn try_propose_adaptive_linear_refit(
    source: &NurbsCurve,
    tolerance: Real,
) -> Result<Option<AdaptiveLinearRefitProposal>, GeometryError> {
    validate(source, tolerance)?;
    let Some(proxy) = source::linear_image_proxy(source)? else {
        return Ok(None);
    };
    pieces::propose(&proxy, tolerance, 1_f64.to_radians())
}

fn validate(source: &NurbsCurve, tolerance: Real) -> Result<(), GeometryError> {
    if !tolerance.is_finite() || tolerance <= 0. {
        return Err(GeometryError::InvalidTolerance);
    }
    if source.control_points().len() > MAX_LOFT_SECTION_CONTROLS {
        return Err(GeometryError::LoftResourceLimit {
            context: "adaptive source controls",
            maximum: MAX_LOFT_SECTION_CONTROLS,
        });
    }
    Ok(())
}

/// Reconstruct curvature-continuous regions of a certified raw linear proposal.
/// This is a geometry proposal, not Rhino-compatible adaptive Loft preparation:
/// native subdivision decisions remain unqualified on uneven source regions.
/// `angle` is in radians in `[0, pi]`; zero uses the one-degree default and
/// positive angles use the measured 0.002-radian resolution floor. The final
/// joined curve requires its own complete bound within `tolerance`, so raw and
/// reconstruction errors are never added beyond the requested tolerance.
/// Limits and failure behavior match [`try_propose_adaptive_linear_refit`].
pub fn try_propose_joined_linear_refit(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    validate(source, tolerance)?;
    validate_angle(Some(angle))?;
    let angle = corner_angle(Some(angle));
    let Some(proxy) = source::linear_image_proxy(source)? else {
        return Ok(None);
    };
    let fitting_source = &proxy;
    let Some(proposal) = pieces::propose(fitting_source, tolerance, angle)? else {
        return Ok(None);
    };
    let result = join::construct(&proposal.curve, angle)?;
    if crate::curve_fit::g1::within_deviation(fitting_source, &result, tolerance)? {
        Ok(Some(result))
    } else {
        Ok(None)
    }
}

mod join;
mod pieces;
mod source;

#[cfg(test)]
mod tests;
