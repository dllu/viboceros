//! Adaptive polynomial proposals; only complete tensor bounds admit a result.
use super::*;
use std::collections::HashMap;
mod refine;
#[cfg(test)]
mod tests;

const MAX_SAMPLES: usize = 262_144;

/// Polynomial degrees and output ceilings for automatic bounded fitting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceFitOptions {
    pub degree: [usize; 2],
    pub maximum_control_count: [usize; 2],
}
impl Default for SurfaceFitOptions {
    fn default() -> Self {
        Self {
            degree: [3; 2],
            maximum_control_count: [MAX_SURFACE_FIT_AXIS; 2],
        }
    }
}

/// Choose a polynomial U/V basis automatically, certifying the entire original
/// active surface at `limit`. Starts with one Bezier patch and bisects selected
/// knot intervals. Sampled residual variation guides proposals; it never admits
/// a result. An inconclusive certificate with no sampled error refines every
/// available interval. Zero is strict. `None` means no candidate was certified
/// within the output/sampling limits, not that approximation is impossible.
///
/// Preserves source UV domains. Simple output knots retain C(degree-1) joins;
/// no trimming, tangent, topology or Rhino adaptive-policy claim is made.
pub fn try_fit_surface_with_bound(
    source: &NurbsSurface,
    options: SurfaceFitOptions,
    limit: Real,
) -> Result<Option<BoundedSurfaceFit>, GeometryError> {
    try_fit_surface_with_bound_with_solver(
        source,
        options,
        limit,
        &mut crate::surface_rebuild::portable_solve,
    )
}

/// Shared automatic basis selection and exact admission with a batched solver.
pub fn try_fit_surface_with_bound_with_solver<E: From<GeometryError>>(
    source: &NurbsSurface,
    options: SurfaceFitOptions,
    limit: Real,
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
) -> Result<Option<BoundedSurfaceFit>, E> {
    fit_with_fallback(source, options, limit, solve, || Ok(None))
}

/// A separate image fitter can propose a certified quad after the first
/// parameter fit declines. The ordinary fitter supplies no such proposal.
pub(super) fn fit_with_fallback<E: From<GeometryError>>(
    source: &NurbsSurface,
    options: SurfaceFitOptions,
    limit: Real,
    solve: &mut impl FnMut(&NurbsInterpolationSystem<'_>) -> Result<Vec<Real>, E>,
    fallback: impl FnOnce() -> Result<Option<BoundedSurfaceFit>, GeometryError>,
) -> Result<Option<BoundedSurfaceFit>, E> {
    validate_source(source)?;
    if !limit.is_finite() || limit < 0. {
        return Err(GeometryError::InvalidTolerance.into());
    }
    if (0..2).any(|a| {
        !(1..=11).contains(&options.degree[a])
            || options.maximum_control_count[a] <= options.degree[a]
            || options.maximum_control_count[a] > MAX_SURFACE_FIT_AXIS
    }) {
        return Err(unsupported(
            "automatic fitting requires degrees 1..=11 and degree+1..=64 controls per axis",
        )
        .into());
    }
    let domains = [source.domain_u(), source.domain_v()];
    let mut fallback = Some(fallback);
    let mut cuts = domains.each_ref().map(|d| vec![*d.start(), *d.end()]);
    let mut cache = HashMap::<(u64, u64), Point3>::new();
    let mut sample = |u: Real, v: Real| -> Result<Point3, GeometryError> {
        let key = (u.to_bits(), v.to_bits());
        if let Some(&point) = cache.get(&key) {
            return Ok(point);
        }
        if cache.len() >= MAX_SAMPLES {
            return Err(GeometryError::SurfaceApproximationLimit);
        }
        let point = source.evaluate(u, v)?;
        cache.insert(key, point);
        Ok(point)
    };
    loop {
        let knots: [Vec<Real>; 2] = std::array::from_fn(|a| {
            [
                vec![cuts[a][0]; options.degree[a]],
                cuts[a].clone(),
                vec![*cuts[a].last().unwrap(); options.degree[a]],
            ]
            .concat()
        });
        let candidate = match super::interpolate_with_sampler(
            source,
            options.degree,
            [&knots[0], &knots[1]],
            solve,
            &mut sample,
        ) {
            Err(SampledFitError::Sample(GeometryError::SurfaceApproximationLimit)) => {
                return Ok(None);
            }
            Err(SampledFitError::Sample(e)) => return Err(e.into()),
            Err(SampledFitError::Backend(e)) => return Err(e),
            Ok(candidate) => candidate,
        };
        let outside = match refine::sampled_outside(&candidate, &cuts, limit, &mut sample) {
            Err(GeometryError::SurfaceApproximationLimit) => return Ok(None),
            result => result?,
        };
        if !outside {
            if let Some(maximum_deviation_bound) =
                source.parameter_surface_deviation_bound(&candidate, limit)?
            {
                return Ok(Some(BoundedSurfaceFit {
                    surface: candidate,
                    maximum_deviation_bound,
                }));
            }
        }
        if let Some(propose) = fallback.take()
            && let Some(result) = propose()?
        {
            return Ok(Some(result));
        }
        let added = match refine::refine(&candidate, &mut cuts, options, limit, &mut sample) {
            Err(GeometryError::SurfaceApproximationLimit) => return Ok(None),
            result => result?,
        };
        if !added {
            return Ok(None);
        }
    }
}
