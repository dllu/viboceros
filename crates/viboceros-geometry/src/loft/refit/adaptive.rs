//! Bounded adaptive cubic proposals with complete final image certificates.
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

#[derive(Clone, Copy)]
pub(super) struct FitPolicy {
    pub(super) closed: bool,
    pub(super) single: bool,
    pub(super) geometric_rebuild: bool,
}

impl FitPolicy {
    pub(super) const LOFT: Self = Self {
        closed: false,
        single: false,
        geometric_rebuild: false,
    };

    pub(super) fn sweep(section_count: usize) -> Self {
        Self {
            closed: true,
            single: section_count == 1,
            geometric_rebuild: true,
        }
    }

    pub(super) fn sample_limit(self, tolerance: Real) -> Real {
        tolerance * if self.single { 1. } else { 0.5 }
    }
}

/// Propose bounded adaptive cubics for exactly extracted straight regions with
/// no resolved one-degree tangent breaks. The source image and limiting frames
/// are retained through an exact, uniformly parameterized polygon proxy.
/// Seven interior stations select pieces using half the requested tolerance;
/// Complete interval bounds also trigger subdivision when stations miss a
/// feature. The assembled result requires a fresh complete image bound within
/// the requested tolerance.
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

/// Propose joined cubics from the original source, retaining its parameter clock.
/// A single nine-station incremental length lookup selects all dyadic splits;
/// interpolation in that lookup is a proposal, not a new length integration.
/// Coincident interval endpoints trigger bounded subdivision, retaining loops
/// within an open source instead of attempting a zero-chord cubic.
/// Seven interior stations from that lookup and complete interval bounds
/// select raw endpoint-frame pieces, with a common midpoint handle correction
/// before subdivision. Intervals crossing an interior C1 break keep a complete
/// half-tolerance reserve. A single curvature region selects fresh dyadic Hermite
/// intervals after retiming, with complete half-tolerance pointwise bounds.
/// C1 reconstruction requires a fresh complete bound against
/// the original source within `tolerance`, even if the raw assembly cannot.
///
/// This geometry proposal does not establish general Rhino adaptive acceptance.
/// The Loft command retains its separately qualified preparation policy.
/// `angle` follows [`try_propose_joined_linear_refit`]. Sources above degree 16,
/// resolved tangent breaks, closed curves or unresolved final proofs return
/// `None`. Resource and failure limits otherwise match
/// [`try_propose_adaptive_linear_refit`].
pub fn try_propose_joined_refit(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
) -> Result<Option<NurbsCurve>, GeometryError> {
    Ok(certified_joined_refit(source, tolerance, angle)?.map(|proposal| proposal.curve))
}

pub(super) struct CertifiedJoinedRefit {
    pub curve: NurbsCurve,
    pub maximum_deviation_bound: Real,
    pub reconstruction_source: Option<NurbsCurve>,
}

pub(super) fn reconstruct_at_cuts(
    source: &NurbsCurve,
    cuts: &[Real],
) -> Result<NurbsCurve, GeometryError> {
    join::reconstruct_at_cuts(source, cuts)
}

pub(super) fn certified_joined_refit(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
) -> Result<Option<CertifiedJoinedRefit>, GeometryError> {
    certified_joined_refit_with_scope(source, tolerance, angle, FitPolicy::LOFT)
}

pub(super) fn certified_joined_refit_with_scope(
    source: &NurbsCurve,
    tolerance: Real,
    angle: Real,
    policy: FitPolicy,
) -> Result<Option<CertifiedJoinedRefit>, GeometryError> {
    validate(source, tolerance)?;
    validate_angle(Some(angle))?;
    if source.degree() > 16 {
        return Ok(None);
    }
    let angle = corner_angle(Some(angle));
    let Some(candidate) = pieces::construct_with_scope(
        source,
        tolerance,
        angle,
        pieces::SplitClock::SourceLookup,
        policy,
    )?
    else {
        return Ok(None);
    };
    let (joined, reconstruction_source) = if policy.single || candidate.preserve_closed_spans {
        (join::construct_smooth(&candidate.curve)?, None)
    } else {
        let Some(refined) = join::construct_refined_with_policy(
            &candidate.curve,
            angle,
            tolerance,
            policy.geometric_rebuild.then_some(source),
        )?
        else {
            return Ok(None);
        };
        (refined.curve, refined.source)
    };
    let copies = [joined];
    let mapped = super::super::parameterize::prepare_fitted(&copies, &mut |curve: &NurbsCurve| {
        curve.clamped_to_active_domain()
    })?;
    let joined = mapped[0].clone();
    let has_regions = joined.knots().chunk_by(|a, b| a == b).any(|g| {
        g.len() == joined.degree()
            && g[0] > *joined.domain().start()
            && g[0] < *joined.domain().end()
    });
    let bound = if has_regions || policy.single {
        let target = joined.knots()[joined.degree()..=joined.control_points().len()]
            .chunk_by(|a, b| a == b)
            .map(|g| g[0])
            .collect::<Vec<_>>();
        let domain = source.domain();
        let target_domain = joined.domain();
        let pairs = candidate
            .source_parameters
            .iter()
            .zip(target)
            .map(|(&t, q)| {
                Ok([
                    crate::nurbs::interval_fraction(t, *domain.start(), *domain.end())?,
                    crate::nurbs::interval_fraction(
                        q,
                        *target_domain.start(),
                        *target_domain.end(),
                    )?,
                ])
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        crate::curve_fit::g1::deviation_bound_at_pairs(source, &joined, tolerance, &pairs)?
    } else if policy.geometric_rebuild {
        crate::curve_fit::g1::reconstruction_deviation_bound(source, &joined, tolerance)?
    } else {
        crate::curve_fit::g1::deviation_bound(source, &joined, tolerance)?
    };
    let bound = if bound.is_none() && policy.single {
        // Retiming changes the target clock. If the original fitting-span
        // stations cannot certify the assembly, propose fresh target-knot
        // witnesses and tangent frames before declining it. The unchanged
        // complete image bound still decides at the requested tolerance.
        crate::curve_fit::g1::reconstruction_deviation_bound(source, &joined, tolerance)?
    } else {
        bound
    };
    Ok(bound.map(|maximum_deviation_bound| CertifiedJoinedRefit {
        curve: joined,
        maximum_deviation_bound,
        reconstruction_source,
    }))
}

mod join;
mod pieces;
mod source;

#[cfg(test)]
mod curved_tests;
#[cfg(test)]
mod reconstruction_tests;
#[cfg(test)]
mod tests;
