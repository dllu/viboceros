//! Reusable outward-rounded distance keys with exact ambiguity resolution.
use super::*;
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

/// A fixed target's distance to one immutable evaluated point. Keys compared
/// together must have been constructed for that same target.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PointDistance {
    point: Point3,
    lower: Real,
    upper: Real,
}

impl PointDistance {
    /// Enclose the exact squared distance. Widen every subtraction, square,
    /// and addition outwards by one float. No libm accuracy bound is needed.
    /// Underflow keeps zero as a lower bound; overflow widens toward infinity.
    pub(crate) fn new(target: Point3, point: Point3) -> Self {
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
        Self {
            point,
            lower,
            upper,
        }
    }

    pub(crate) fn point(&self) -> Point3 {
        self.point
    }

    pub(crate) fn compare(&self, other: &Self, target: Point3) -> Ordering {
        if self.upper < other.lower {
            Ordering::Less
        } else if other.upper < self.lower {
            Ordering::Greater
        } else {
            target.compare_distances(self.point, other.point)
        }
    }
}
