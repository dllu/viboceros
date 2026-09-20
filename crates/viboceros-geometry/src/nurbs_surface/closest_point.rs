//! Bounded surface closest-point search with affine and curvature-aware refinement.
use super::evaluate::SurfaceQuery;
use super::*;
mod affine;
mod candidate;
mod refine;
use candidate::{Candidate, retain_closest_seeds};
#[cfg(test)]
mod tests;

impl NurbsSurface {
    /// Finds natural surface parameters nearest to a finite model-space
    /// point. Exactly affine bilinear patches use direct constrained projection;
    /// other surfaces use bounded multi-start curvature-aware Newton refinement
    /// with a tangent-plane fallback and local stationarity polishing.
    /// Neither path assumes normalized parameter domains.
    pub fn closest_parameters(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        if let Some(parameters) = self.closest_affine_parameters(target) {
            return Ok(parameters);
        }
        self.closest_parameters_general(target, tolerance)
    }

    fn closest_parameters_general(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        let u_domain = self.domain_u();
        let v_domain = self.domain_v();
        let u_start = *u_domain.start();
        let u_end = *u_domain.end();
        let v_start = *v_domain.start();
        let v_end = *v_domain.end();
        let mut query = SurfaceQuery::new(self);
        // This is the first grid station and wins any equal-distance tie.
        // A coordinate-equal image attains the global lower bound zero; model
        // tolerance or a rounded distance of zero is not sufficient.
        if query
            .evaluate(u_start, v_start)
            .is_ok_and(|point| point == target)
        {
            return Ok((u_start, v_start));
        }
        let u_seeds = closest_parameter_seeds(self.spans_u(), u_start, u_end);
        let v_seeds = closest_parameter_seeds(self.spans_v(), v_start, v_end);
        let mut seeds = Vec::with_capacity(u_seeds.len() * v_seeds.len());
        self.for_each_grid_point(&u_seeds, &v_seeds, |i, j, result| {
            if let Ok(point) = result {
                seeds.push(Candidate::new(target, point, (u_seeds[i], v_seeds[j])));
            }
        });
        // Rank the stored points before discarding starts. Rounded distances
        // can tie (or overflow) while the best basins are still distinguishable.
        // Ties retain the original V-major/U-minor grid order.
        retain_closest_seeds(&mut seeds, target);
        let first = seeds.first().ok_or(GeometryError::Degenerate {
            context: "NURBS surface closest-point search",
        })?;
        let mut best = Candidate::evaluate(&mut query, target, first.parameters)?;
        if best.point == target {
            return Ok(best.parameters);
        }
        // Clamping a coupled two-parameter Newton step can stall before the
        // minimum along an active boundary. Solve all four natural boundary
        // curves independently, including their endpoints. Singular constant
        // edges remain represented by the boundary seeds above.
        for (curve, fixed_u, fixed) in [
            (self.isocurve_u(v_start), false, v_start),
            (self.isocurve_u(v_end), false, v_end),
            (self.isocurve_v(u_start), true, u_start),
            (self.isocurve_v(u_end), true, u_end),
        ] {
            if let Ok(curve) = curve
                && let Ok(t) = curve.closest_parameter(target, tolerance)
                // Extraction can round control points or weights differently.
                // The curve proposes parameters, never the comparison point.
                && let Ok(candidate) = Candidate::evaluate(
                    &mut query,
                    target,
                    if fixed_u { (fixed, t) } else { (t, fixed) },
                )
                && candidate.compare(&best, target).is_lt()
            {
                best = candidate;
                if best.point == target {
                    return Ok(best.parameters);
                }
            }
        }
        let mut refined = false;
        let mut nonfinite_step = false;
        for seed in seeds {
            match query
                .refine_closest_parameters(
                    target,
                    seed.parameters.0,
                    seed.parameters.1,
                    [u_start, u_end],
                    [v_start, v_end],
                    tolerance,
                )
                .and_then(|parameters| Candidate::evaluate(&mut query, target, parameters))
            {
                Ok(candidate) => {
                    refined = true;
                    if candidate.compare(&best, target).is_lt() {
                        best = candidate;
                        if best.point == target {
                            return Ok(best.parameters);
                        }
                    }
                }
                Err(GeometryError::NonFinite { .. }) => nonfinite_step = true,
                Err(_) => {}
            }
        }
        // Native derivatives/steps can be unrepresentable even though the
        // surface and its closest point are finite. Retry in unit domains only
        // on numerical failure, avoiding a control-net copy on ordinary queries.
        // The unit-domain guard makes this fallback non-recursive in practice.
        if (!refined || nonfinite_step)
            && (u_domain != (0.0..=1.0) || v_domain != (0.0..=1.0))
            && let Ok(normalized) = self.try_reparameterized(0.0..=1.0, 0.0..=1.0)
            && let Ok((u, v)) = normalized.closest_parameters(target, tolerance)
            && let (Ok(u), Ok(v)) = (self.parameter_at_u(u), self.parameter_at_v(v))
            && let Ok(candidate) = Candidate::evaluate(&mut query, target, (u, v))
            && candidate.compare(&best, target).is_lt()
        {
            best = candidate;
        }
        Ok(query.polish_closest_parameters(target, best.parameters, tolerance))
    }
}

fn closest_parameter_seeds(
    spans: impl Iterator<Item = (Real, Real)>,
    domain_start: Real,
    domain_end: Real,
) -> Vec<Real> {
    const MAX_SEEDS: usize = 33;
    let spans = spans.collect::<Vec<_>>();
    let mut seeds = Vec::new();
    if spans.len() <= 10 {
        for (start, end) in spans {
            seeds.extend([start, start * 0.5 + end * 0.5, end]);
        }
    }
    let remaining = MAX_SEEDS.saturating_sub(seeds.len()).max(2);
    for index in 0..remaining {
        let fraction = index as Real / (remaining - 1) as Real;
        seeds.push(domain_start.mul_add(1.0 - fraction, domain_end * fraction));
    }
    seeds.sort_by(Real::total_cmp);
    seeds.dedup();
    seeds
}
