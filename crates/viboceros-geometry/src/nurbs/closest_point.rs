//! Bounded NURBS closest-point search and curvature-aware refinement.
use super::evaluate::CurveQuery;
use super::*;
use crate::point::PointDistance;
mod refine;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Candidate {
    parameter: Real,
    distance: PointDistance,
}

impl Candidate {
    fn new(
        query: &mut CurveQuery<'_>,
        target: Point3,
        parameter: Real,
    ) -> Result<Self, GeometryError> {
        Ok(Self {
            parameter,
            distance: PointDistance::new(target, query.evaluate(parameter)?),
        })
    }

    fn compare(&self, other: &Self, target: Point3) -> std::cmp::Ordering {
        self.distance
            .compare(&other.distance, target)
            .then_with(|| self.parameter.total_cmp(&other.parameter))
    }
}

impl NurbsCurve {
    /// Finds an active-domain parameter nearest to a finite model-space point.
    ///
    /// Each nonempty span contributes endpoint/midpoint seeds, supplemented by
    /// a uniform set. Every seed gets curvature-aware Newton refinement with a
    /// tangent fallback. This is a bounded numerical search, not a certified
    /// global rational minimum.
    pub fn closest_parameter(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        // Local coordinates improve stationarity refinement, but translating
        // controls and the target can round away a meaningful offset. The
        // local curve only proposes parameters; rank their original images.
        let origin = self.control_points[0].point;
        if origin.to_array() != [0.0; 3] {
            let offset = Vector3::try_new(-origin.x(), -origin.y(), -origin.z())?;
            if let (Ok(local), Ok(local_target)) = (
                self.transformed(AffineTransform3::from_translation(offset)),
                target.translated(offset),
            ) {
                return self.closest_parameter_with_refinement(
                    target,
                    tolerance,
                    &local,
                    local_target,
                );
            }
        }
        self.closest_parameter_with_refinement(target, tolerance, self, target)
    }

    fn closest_parameter_with_refinement(
        &self,
        target: Point3,
        tolerance: Tolerance,
        refinement: &NurbsCurve,
        refinement_target: Point3,
    ) -> Result<Real, GeometryError> {
        let domain = [*self.domain().start(), *self.domain().end()];
        let mut query = CurveQuery::new(self);
        let mut refinement = CurveQuery::new(refinement);
        let seeds = curve_closest_parameter_seeds(self.spans(), domain[0], domain[1]);
        let mut candidates = Vec::with_capacity(seeds.len());
        for parameter in seeds {
            if let Ok(candidate) = Candidate::new(&mut query, target, parameter) {
                // The first native parameter wins every possible distance tie.
                // Never use model tolerance or a rounded distance for this hit.
                if parameter == domain[0] && candidate.distance.point() == target {
                    return Ok(parameter);
                }
                candidates.push(candidate);
            }
        }
        // Coarse seed distances cannot safely cull a span. A stationary span
        // may supply many closer seeds than the span containing the minimum;
        // an unusually long segment may have distant endpoints and midpoint
        // even when its interior passes exactly through the target.
        let order = |a: &Candidate, b: &Candidate| a.compare(b, target);
        let mut best = candidates
            .iter()
            .min_by(|a, b| order(a, b))
            .copied()
            .ok_or(GeometryError::Degenerate {
                context: "NURBS curve closest-point search",
            })?;
        for seed in candidates {
            if let Ok(parameter) = refinement.refine_closest_parameter_only(
                refinement_target,
                seed.parameter,
                domain,
                tolerance,
            ) && let Ok(candidate) = Candidate::new(&mut query, target, parameter)
                && candidate.compare(&best, target).is_lt()
            {
                best = candidate;
            }
        }
        Ok(best.parameter)
    }

    // Intersection snapping explicitly needs a finite distance as well as a
    // parameter. Keep that requirement out of the general closest-point search.
    pub(super) fn refine_closest_parameter(
        &self,
        target: Point3,
        parameter: Real,
        domain: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        let parameter = CurveQuery::new(self)
            .refine_closest_parameter_only(target, parameter, domain, tolerance)?;
        Ok((parameter, self.evaluate(parameter)?.distance_to(target)?))
    }
}

fn curve_closest_parameter_seeds(
    spans: impl Iterator<Item = (Real, Real)>,
    domain_start: Real,
    domain_end: Real,
) -> Vec<Real> {
    const UNIFORM_SEED_COUNT: usize = 33;
    let mut seeds = Vec::new();
    for (start, end) in spans {
        seeds.extend([start, start * 0.5 + end * 0.5, end]);
    }
    for index in 0..UNIFORM_SEED_COUNT {
        let fraction = index as Real / (UNIFORM_SEED_COUNT - 1) as Real;
        seeds.push(domain_start.mul_add(1.0 - fraction, domain_end * fraction));
    }
    seeds.sort_by(Real::total_cmp);
    seeds.dedup();
    seeds
}
