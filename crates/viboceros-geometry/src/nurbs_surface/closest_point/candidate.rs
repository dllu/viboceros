//! Surface images and conservative distance bounds for candidate ordering.
use super::*;
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(super) struct Candidate {
    pub(super) parameters: (Real, Real),
    pub(super) point: Point3,
    distance: DistanceBounds,
}

impl Candidate {
    pub(super) fn new(target: Point3, point: Point3, parameters: (Real, Real)) -> Self {
        Self {
            point,
            parameters,
            distance: DistanceBounds::new(target, point),
        }
    }

    pub(super) fn evaluate(
        query: &mut SurfaceQuery<'_>,
        target: Point3,
        parameters: (Real, Real),
    ) -> Result<Self, GeometryError> {
        Ok(Self::new(
            target,
            query.evaluate(parameters.0, parameters.1)?,
            parameters,
        ))
    }

    /// Disjoint bounds certify the order cheaply. Overlapping or overflowing
    /// bounds use the exact squared-distance predicate on stored coordinates.
    pub(super) fn compare(&self, other: &Self, target: Point3) -> Ordering {
        if self.distance.upper < other.distance.lower {
            Ordering::Less
        } else if other.distance.upper < self.distance.lower {
            Ordering::Greater
        } else {
            target.compare_distances(self.point, other.point)
        }
    }
}

/// Retain the same sixteen starts as a full exact stable grid sort, without
/// sorting every discarded cell. Grid parameters are unique and ascending in
/// each axis, so V then U explicitly reproduces their original tie order.
pub(super) fn retain_closest_seeds(seeds: &mut Vec<Candidate>, target: Point3) {
    let order = |a: &Candidate, b: &Candidate| {
        a.compare(b, target)
            .then_with(|| a.parameters.1.total_cmp(&b.parameters.1))
            .then_with(|| a.parameters.0.total_cmp(&b.parameters.0))
    };
    const STARTS: usize = 16;
    if seeds.len() > STARTS {
        seeds.select_nth_unstable_by(STARTS, order);
        seeds.truncate(STARTS);
    }
    seeds.sort_unstable_by(order);
}

#[derive(Clone, Copy, Debug)]
struct DistanceBounds {
    lower: Real,
    upper: Real,
}

impl DistanceBounds {
    /// Encloses the exact squared distance between finite binary64 points.
    /// Widen every subtraction, square, and addition outwards by one float.
    /// This needs neither a square root nor assumptions about libm's accuracy.
    /// Underflow keeps zero as a lower bound; overflow widens toward infinity.
    fn new(target: Point3, point: Point3) -> Self {
        let mut lower = 0.;
        let mut upper = 0.;
        for (a, b) in target.to_array().into_iter().zip(point.to_array()) {
            if a == b {
                continue;
            }
            let delta = a - b;
            let lo = delta.next_down();
            let hi = delta.next_up();
            let min_abs = if lo <= 0. && hi >= 0. {
                0.
            } else {
                lo.abs().min(hi.abs())
            };
            let max_abs = lo.abs().max(hi.abs());
            let square_lo = (min_abs * min_abs).next_down().max(0.);
            let square_hi = (max_abs * max_abs).next_up();
            lower = (lower + square_lo).next_down().max(0.);
            upper = (upper + square_hi).next_up();
        }
        Self { lower, upper }
    }
}
