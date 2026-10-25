//! Count local knot-side probes rather than multiplying complete tensor tables.
use super::*;

pub(super) fn needed(
    surface: &NurbsSurface,
    continuity: CurveContinuity,
    budget: &mut usize,
) -> Result<bool, GeometryError> {
    let axes = [
        (surface.degree_u(), surface.knots_u(), surface.domain_u()),
        (surface.degree_v(), surface.knots_v(), surface.domain_v()),
    ];
    let mut candidates = [0usize; 2];
    let mut intervals = [0usize; 2];
    for (axis, (degree, knots, domain)) in axes.iter().enumerate() {
        charge(budget, knots.len())?;
        // The native traversal skips knots whose basis continuity already
        // exceeds the requested order.
        let minimum = if continuity == CurveContinuity::G1 {
            *degree
        } else {
            degree.saturating_sub(1).max(1)
        };
        candidates[axis] = knots
            .chunk_by(|a, b| a == b)
            .filter(|g| g.len() >= minimum && g[0] > *domain.start() && g[0] < *domain.end())
            .count();
        // Native traversal also samples zero-width intervals at repeated
        // knots. Count every active interval rather than only nonempty spans.
        intervals[axis] = knots.len().saturating_sub(2 * degree + 1);
    }
    if candidates == [0, 0] {
        return Ok(false);
    }
    let local = (surface.degree_u() + 1)
        .saturating_mul(surface.degree_v() + 1)
        .saturating_mul(12);
    let probes = candidates[0]
        .saturating_mul(intervals[1].saturating_mul(2).saturating_add(1))
        .saturating_add(
            candidates[1].saturating_mul(intervals[0].saturating_mul(2).saturating_add(1)),
        );
    charge(
        budget,
        surface
            .control_points()
            .len()
            .saturating_mul(4)
            .saturating_add(probes.saturating_mul(local)),
    )?;
    Ok(true)
}
