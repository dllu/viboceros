//! Continuous correspondence of a tensor surface image and a spatial spline.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{One, Zero};
use std::collections::BTreeMap;

mod algebra;
mod curve;
mod surface;
#[cfg(test)]
mod tests;

type H = [Rational; 4];
type Uv = [Rational; 3];
type Net<const D: usize> = Vec<[Rational; D]>;
const MAX_DEGREE: usize = 16;
const MAX_IMAGE_DEGREE: usize = 64;
const MAX_DEPTH: usize = 48;
const MAX_WORK: usize = 2_000_000;
const MAX_BITS: u64 = 8192;

struct Budget(usize);
impl Budget {
    fn charge(&mut self, n: usize) -> Result<(), GeometryError> {
        self.0 = self
            .0
            .checked_sub(n)
            .ok_or(GeometryError::SurfaceCurveCertificateWorkLimit)?;
        Ok(())
    }
    fn check(&mut self, r: &Rational) -> Result<(), GeometryError> {
        self.charge(1)?;
        if r.numer().bits() > MAX_BITS || r.denom().bits() > MAX_BITS {
            return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
        }
        Ok(())
    }
}

impl NurbsSurface {
    /// Certifies a continuous model-space error bound between `self(uv(t))`
    /// and `spatial(t)`, with both curve domains mapped affinely to [0,1].
    /// `Some(bound)` proves the entire correspondence is within `limit`.
    /// `None` is inconclusive or outside that limit, never proof of equality.
    ///
    /// Exact rational extraction and Bernstein products retain all stored
    /// controls, weights and knots without rounded knot insertion or sampling.
    /// Tensor knot crossings use restricted surface hulls and exact dyadic
    /// subdivision. Positive or uniformly negative weight gauges are supported;
    /// mixed signs, interior full-order surface knots, degrees above 16, and
    /// composed degrees above 64 are uncertified. UV must stay in the natural
    /// surface domain. Work, rational sizes and subdivision depth are bounded.
    /// Sources remain unchanged; this does not certify topology or injectivity.
    pub fn parameter_curve_deviation_bound(
        &self,
        uv: &NurbsCurve2,
        spatial: &NurbsCurve,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if !limit.is_finite() || limit < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        let Some(mut certificate) = PullbackCertificate::with_degree(self, spatial, uv.degree())?
        else {
            return Ok(None);
        };
        certificate.curve(uv, limit)
    }

    /// Discovers a certified straight UV path when possible, otherwise fits
    /// a regular pullback, then proves its complete model-space image is
    /// within the caller's absolute tolerance of the original spatial curve.
    /// An inconclusive certificate returns an error; sampled acceptance alone
    /// is insufficient. Parameter-domain and certificate limits are those of
    /// [`Self::try_pullback_curve`] and [`Self::parameter_curve_deviation_bound`].
    pub fn try_pullback_curve_certified(
        &self,
        spatial: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve2, GeometryError> {
        self.certified_pullback(spatial, tolerance, None)
    }

    /// Pulls back a spatial curve with both UV endpoints fixed. Exact inverse
    /// proposals retain their control structure when an endpoint adjustment
    /// certifies. A straight UV proposal can unwrap a closed spatial isocurve
    /// across a periodic seam. Otherwise cubic fitting incorporates the fixed
    /// endpoints into its nodes and refines until the complete image certifies
    /// at absolute tolerance. The spatial curve and its domain are unchanged.
    ///
    /// Endpoints must lie in the natural surface domain and within model-space
    /// tolerance of the respective spatial endpoints. This proves normalized
    /// parameter correspondence, not trim simplicity or topology. The regular
    /// fitter and certificate resource limits also apply.
    pub fn try_pullback_curve_certified_with_endpoints(
        &self,
        spatial: &NurbsCurve,
        endpoints: [Point2; 2],
        tolerance: Tolerance,
    ) -> Result<NurbsCurve2, GeometryError> {
        self.certified_pullback(spatial, tolerance, Some(endpoints))
    }

    fn certified_pullback(
        &self,
        spatial: &NurbsCurve,
        tolerance: Tolerance,
        endpoints: Option<[Point2; 2]>,
    ) -> Result<NurbsCurve2, GeometryError> {
        self.try_pullback_curve_certified_with_bound(spatial, endpoints, tolerance)
            .map(|(uv, _)| uv)
    }

    /// Returns a certified pullback and its complete model-space deviation
    /// bound. Optional endpoints have the same meaning as
    /// [`Self::try_pullback_curve_certified_with_endpoints`]. Already certified
    /// inverse and straight proposals retain their proof; assembled Hermite
    /// fits receive a final certificate across all original source knot spans.
    /// This avoids repeating exact arithmetic merely to report the bound.
    pub fn try_pullback_curve_certified_with_bound(
        &self,
        spatial: &NurbsCurve,
        endpoints: Option<[Point2; 2]>,
        tolerance: Tolerance,
    ) -> Result<(NurbsCurve2, Real), GeometryError> {
        let (uv, bound) = self.pullback_curve(spatial, tolerance, true, endpoints)?;
        let bound = match bound {
            Some(bound) => bound,
            None => self
                .parameter_curve_deviation_bound(&uv, spatial, tolerance.absolute())?
                .ok_or(GeometryError::SurfacePullbackDidNotConverge {
                    tolerance: tolerance.absolute(),
                })?,
        };
        Ok((uv, bound))
    }
}

fn supported(surface: &NurbsSurface, uv_degree: usize, spatial_degree: usize) -> bool {
    uv_degree <= MAX_DEGREE
        && spatial_degree <= MAX_DEGREE
        && surface.degree_u() <= MAX_DEGREE
        && surface.degree_v() <= MAX_DEGREE
        && uv_degree.saturating_mul(surface.degree_u() + surface.degree_v()) <= MAX_IMAGE_DEGREE
}

/// Reuses the original exact spatial spline, tensor patches, and one budget for
/// all proposed fitter segments. Spatial intervals are extracted exactly; a
/// rounded `try_trimmed` curve must never replace the source being certified.
pub(super) struct PullbackCertificate {
    surface: surface::Surface,
    spatial: curve::Spline<4>,
    domain: [Rational; 2],
    budget: Budget,
    uv_degree: usize,
}

impl PullbackCertificate {
    pub(super) fn new(
        surface: &NurbsSurface,
        spatial: &NurbsCurve,
    ) -> Result<Option<Self>, GeometryError> {
        Self::with_degree(surface, spatial, PULLBACK_DEGREE)
    }

    pub(super) fn with_degree(
        surface: &NurbsSurface,
        spatial: &NurbsCurve,
        uv_degree: usize,
    ) -> Result<Option<Self>, GeometryError> {
        if !supported(surface, uv_degree, spatial.degree()) {
            return Ok(None);
        }
        let mut budget = Budget(MAX_WORK);
        let Some(surface) = surface::Surface::new(surface, &mut budget)? else {
            return Ok(None);
        };
        let Some(exact) = curve::Spline::spatial(spatial, &mut budget)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            surface,
            spatial: exact,
            domain: [
                rational(*spatial.domain().start()),
                rational(*spatial.domain().end()),
            ],
            budget,
            uv_degree,
        }))
    }

    /// Reuses exact reference extraction, tensor patches and one work budget
    /// across complete proposals. Each proposal keeps normalized correspondence
    /// to the original source, including every source and UV knot span.
    pub(super) fn curve(
        &mut self,
        uv: &NurbsCurve2,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if uv.degree() != self.uv_degree {
            return Ok(None);
        }
        let Some(uv) = curve::Spline::uv(uv, &mut self.budget)? else {
            return Ok(None);
        };
        let mut cuts = uv.cuts();
        cuts.sort();
        cuts.dedup();
        if self.uv_degree == 1 {
            let mut crossings = Vec::new();
            for interval in cuts.windows(2) {
                let controls = uv.extract(&interval[0], &interval[1], &mut self.budget)?;
                for fraction in self.surface.linear_crossings(&controls, &mut self.budget)? {
                    let t = &interval[0] + (&interval[1] - &interval[0]) * fraction;
                    self.budget.check(&t)?;
                    crossings.push(t);
                }
            }
            cuts.extend(crossings);
        }
        cuts.extend(self.spatial.cuts());
        cuts.sort();
        cuts.dedup();
        let mut bound = 0_f64;
        for interval in cuts.windows(2) {
            let uv = uv.extract(&interval[0], &interval[1], &mut self.budget)?;
            let spatial = self
                .spatial
                .extract(&interval[0], &interval[1], &mut self.budget)?;
            let Some(upper) = piece_bound(&mut self.surface, uv, spatial, limit, &mut self.budget)?
            else {
                return Ok(None);
            };
            bound = bound.max(upper);
        }
        Ok(Some(bound))
    }

    pub(super) fn segment(
        &mut self,
        segment: PullbackSegment,
        limit: Real,
    ) -> Result<bool, GeometryError> {
        let width = &self.domain[1] - &self.domain[0];
        let start = (rational(segment.start) - &self.domain[0]) / &width;
        let end = (rational(segment.end) - &self.domain[0]) / &width;
        // append_span subdivides one original spatial knot span at a time.
        let spatial = self.spatial.extract(&start, &end, &mut self.budget)?;
        self.budget.charge(3 * segment.controls.len())?;
        let uv = segment
            .controls
            .iter()
            .map(|p| [rational(p.x()), rational(p.y()), Rational::one()])
            .collect();
        Ok(piece_bound(&mut self.surface, uv, spatial, limit, &mut self.budget)?.is_some())
    }

    /// Certifies a polynomial cubic on a whole-domain fractional interval.
    /// Its sampling times need not be representable in the native knot domain.
    /// Every original spatial knot inside the interval is retained explicitly.
    pub(super) fn fractional_segment(
        &mut self,
        interval: [Real; 2],
        controls: [Point2; 4],
        limit: Real,
    ) -> Result<bool, GeometryError> {
        let [start, end] = interval.map(rational);
        let width = &end - &start;
        let mut cuts = self
            .spatial
            .cuts()
            .into_iter()
            .filter(|t| *t > start && *t < end)
            .collect::<Vec<_>>();
        cuts.extend([start.clone(), end.clone()]);
        cuts.sort();
        cuts.dedup();
        let uv = controls.map(|p| [rational(p.x()), rational(p.y()), Rational::one()]);
        let knots = [vec![Rational::zero(); 4], vec![Rational::one(); 4]].concat();
        for bounds in cuts.windows(2) {
            let first = (&bounds[0] - &start) / &width;
            let last = (&bounds[1] - &start) / &width;
            let uv = curve::extract(&knots, 3, 3, &uv, &first, &last, &mut self.budget)?;
            let spatial = self
                .spatial
                .extract(&bounds[0], &bounds[1], &mut self.budget)?;
            if piece_bound(&mut self.surface, uv, spatial, limit, &mut self.budget)?.is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn piece_bound(
    surface: &mut surface::Surface,
    uv: Vec<Uv>,
    spatial: Vec<H>,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    let mut bound = 0_f64;
    let mut pending = vec![(uv, spatial, 0)];
    while let Some((uv, spatial, depth)) = pending.pop() {
        let bounds = curve::bounds(&uv, budget)?;
        if !surface.in_domain(&bounds) {
            if depth == MAX_DEPTH || !surface.endpoints_in_domain(&uv) {
                return Ok(None);
            }
        } else if let Some(patch) = surface.containing_patch(&bounds, budget)? {
            let image = surface.compose(patch, &uv, budget)?;
            let difference = algebra::difference(&image, &spatial, budget)?;
            let Some(upper) = algebra::hull_bound(difference, limit, budget)? else {
                return Ok(None);
            };
            bound = bound.max(upper);
            continue;
        } else if let Some(upper) = surface.crossing_bound(&bounds, &spatial, limit, budget)? {
            bound = bound.max(upper);
            continue;
        }
        if depth == MAX_DEPTH {
            return Ok(None);
        }
        let (ua, ub) = algebra::split(&uv, budget)?;
        let (sa, sb) = algebra::split(&spatial, budget)?;
        pending.push((ub, sb, depth + 1));
        pending.push((ua, sa, depth + 1));
    }
    Ok(Some(bound))
}
