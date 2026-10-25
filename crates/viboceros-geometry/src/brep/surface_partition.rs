//! Adaptive rectangular tensor partitions, including shared T junctions.
use super::*;
use crate::CurveContinuity;

mod query_budget;
#[cfg(test)]
mod tests;
mod topology;

const MAX_FACES: usize = 4096;
const MAX_WORK: usize = 16_000_000;

impl Brep {
    /// Construct a new tensor shell, splitting each restricted surface at the
    /// requested public OpenNURBS isocurve continuity breaks. A crease may end
    /// at another edge. The query samples transverse spans; it is not a
    /// continuous maximum-angle certificate. Faces retain their native charts,
    /// and share original-surface isocurves and sewn closed/singular boundaries.
    /// Accepts at most three criteria and 4096 faces within a shared work budget.
    /// Full-order interior knots are rejected rather than sewn across jumps.
    /// This constructs fresh topology and does not edit an existing trimmed shell.
    pub fn try_surface_partitioned_at_discontinuities(
        surface: &NurbsSurface,
        criteria: &[(CurveContinuity, Real)],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if criteria.len() > 3 {
            return invalid("surface partition supports at most three continuity criteria");
        }
        if criteria
            .iter()
            .any(|(_, a)| !a.is_finite() || !(0. ..=std::f64::consts::PI).contains(a))
        {
            return Err(GeometryError::InvalidContinuityAngle);
        }
        for (degree, knots, domain) in [
            (surface.degree_u(), surface.knots_u(), surface.domain_u()),
            (surface.degree_v(), surface.knots_v(), surface.domain_v()),
        ] {
            if knots
                .chunk_by(|a, b| a == b)
                .any(|g| g.len() > degree && domain.start() < &g[0] && &g[0] < domain.end())
            {
                return invalid("surface partition cannot sew full-order positional breaks");
            }
        }
        let mut budget = MAX_WORK;
        charge(&mut budget, surface.control_points().len())?;
        let mut cells = vec![surface.clone()];
        let mut index = 0;
        while index < cells.len() && !criteria.is_empty() {
            let current = &cells[index];
            let mut candidates = [Vec::new(), Vec::new()];
            for &(continuity, angle) in criteria {
                if !query_budget::needed(current, continuity, &mut budget)? {
                    continue;
                }
                for (all, cuts) in candidates
                    .iter_mut()
                    .zip(current.discontinuity_parameters(continuity, angle)?)
                {
                    all.extend(cuts);
                }
            }
            let cut = candidates
                .into_iter()
                .enumerate()
                .find_map(|(axis, mut cuts)| {
                    cuts.sort_by(Real::total_cmp);
                    cuts.first().copied().map(|t| (axis, t))
                });
            let Some((axis, cut)) = cut else {
                index += 1;
                continue;
            };
            if cells.len() >= MAX_FACES {
                return Err(GeometryError::SurfaceGridResourceLimit { maximum: MAX_FACES });
            }
            charge(
                &mut budget,
                current.control_points().len().saturating_mul(8),
            )?;
            let u = current.domain_u();
            let v = current.domain_v();
            let (low, high) = if axis == 0 {
                (
                    current.try_trimmed(*u.start()..=cut, v.clone())?,
                    current.try_trimmed(cut..=*u.end(), v)?,
                )
            } else {
                (
                    current.try_trimmed(u.clone(), *v.start()..=cut)?,
                    current.try_trimmed(u, cut..=*v.end())?,
                )
            };
            // Retain low U or high V, as in the general face splitter.
            let (first, last) = if axis == 0 { (low, high) } else { (high, low) };
            cells[index] = first;
            cells.push(last);
        }
        topology::build(surface, cells, tolerance, &mut budget)
    }
}

fn charge(budget: &mut usize, amount: usize) -> Result<(), GeometryError> {
    *budget = budget
        .checked_sub(amount)
        .ok_or(GeometryError::InvalidBrepTopology {
            context: "surface partition exceeds its work budget",
        })?;
    Ok(())
}
