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
        certificate(self, uv, spatial, limit, &mut Budget(MAX_WORK))
    }

    /// Fits a regular pullback, then proves its complete model-space image is
    /// within the caller's absolute tolerance of the original spatial curve.
    /// An inconclusive certificate returns an error; sampled acceptance alone
    /// is insufficient. Parameter-domain and certificate limits are those of
    /// [`Self::try_pullback_curve`] and [`Self::parameter_curve_deviation_bound`].
    pub fn try_pullback_curve_certified(
        &self,
        spatial: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve2, GeometryError> {
        let uv = self.try_pullback_curve(spatial, tolerance)?;
        if self
            .parameter_curve_deviation_bound(&uv, spatial, tolerance.absolute())?
            .is_none()
        {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: tolerance.absolute(),
            });
        }
        Ok(uv)
    }
}

fn certificate(
    surface: &NurbsSurface,
    uv: &NurbsCurve2,
    spatial: &NurbsCurve,
    limit: Real,
    budget: &mut Budget,
) -> Result<Option<Real>, GeometryError> {
    if uv.degree() > MAX_DEGREE
        || spatial.degree() > MAX_DEGREE
        || surface.degree_u() > MAX_DEGREE
        || surface.degree_v() > MAX_DEGREE
        || uv
            .degree()
            .saturating_mul(surface.degree_u() + surface.degree_v())
            > MAX_IMAGE_DEGREE
    {
        return Ok(None);
    }
    let Some(uv) = curve::Spline::uv(uv, budget)? else {
        return Ok(None);
    };
    let Some(spatial) = curve::Spline::spatial(spatial, budget)? else {
        return Ok(None);
    };
    let Some(mut surface) = surface::Surface::new(surface, budget)? else {
        return Ok(None);
    };
    let mut cuts = uv.cuts();
    cuts.extend(spatial.cuts());
    cuts.sort();
    cuts.dedup();
    let mut bound = 0_f64;
    for interval in cuts.windows(2) {
        let uv = uv.extract(&interval[0], &interval[1], budget)?;
        let spatial = spatial.extract(&interval[0], &interval[1], budget)?;
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
    }
    Ok(Some(bound))
}
