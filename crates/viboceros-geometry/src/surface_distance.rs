//! Certified point distance to a complete untrimmed tensor surface.
use crate::{Point3, Real};

/// A near-minimum from a finite point to the complete active NURBS surface.
/// The exact minimum lies in `[upper_distance - maximum_error, upper_distance]`.
/// The upper bound applies to exact evaluation at the native UV parameters;
/// `point` is rounded for display and need not attain that rounded distance.
/// This does not certify parameter uniqueness, topology, or trimmed-face membership.
#[derive(Clone, Debug)]
pub struct SurfaceMinimumDistance {
    pub parameters: [Real; 2],
    pub point: Point3,
    pub upper_distance: Real,
    pub maximum_error: Real,
}

impl SurfaceMinimumDistance {
    /// A downward-rounded nonnegative lower bound for the true minimum.
    pub fn lower_distance_bound(&self) -> Real {
        if self.upper_distance <= self.maximum_error {
            0.
        } else {
            (self.upper_distance - self.maximum_error)
                .next_down()
                .max(0.)
        }
    }
}
