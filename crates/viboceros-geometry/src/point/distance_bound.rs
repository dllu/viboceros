//! Reuse the exact distance predicate from the curve correspondence certificates.
use super::*;
use crate::binary_accumulator::pair_products::Products;
use std::cmp::Ordering;

impl Point3 {
    /// An outward Euclidean distance bound within `limit`, or None when the
    /// exact distance exceeds it. Acceptance does not depend on libm accuracy.
    pub fn distance_bound_to(
        self,
        other: Self,
        limit: Real,
    ) -> Result<Option<Real>, GeometryError> {
        if !limit.is_finite() || limit < 0. {
            return Err(GeometryError::InvalidTolerance);
        }
        Ok(point_bound(self, other, limit))
    }
}

pub(crate) fn point_bound(a: Point3, b: Point3, limit: Real) -> Option<Real> {
    if a == b {
        return Some(0.);
    }
    let mut squared = Products::default();
    for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
        squared.add(a, a, false);
        squared.add(b, b, false);
        squared.add(a, b, true);
        squared.add(a, b, true);
    }
    if squared.compare_square(limit) == Ordering::Greater {
        return None;
    }
    let mut bound = a.distance_to(b).unwrap_or(limit).min(limit);
    for _ in 0..4 {
        if squared.compare_square(bound) != Ordering::Greater {
            return Some(bound);
        }
        bound = bound.next_up().min(limit);
    }
    Some(limit)
}

#[test]
fn distance_bounds_keep_exact_thresholds_across_the_finite_range() {
    let p = |v: [f64; 3]| Point3::try_from(v).unwrap();
    let zero = p([0.; 3]);
    assert_eq!(
        zero.distance_bound_to(p([3., 4., 0.]), 5.).unwrap(),
        Some(5.)
    );
    assert_eq!(
        zero.distance_bound_to(p([3., 4., 0.]), 5_f64.next_down())
            .unwrap(),
        None
    );
    let tiny = f64::from_bits(1);
    assert_eq!(
        zero.distance_bound_to(p([tiny, tiny, 0.]), tiny).unwrap(),
        None
    );
    assert_eq!(
        zero.distance_bound_to(p([tiny, tiny, 0.]), 2. * tiny)
            .unwrap(),
        Some(2. * tiny)
    );
    let huge = 2_f64.powi(1023);
    assert_eq!(
        p([-huge, 0., 0.])
            .distance_bound_to(p([huge, 0., 0.]), f64::MAX)
            .unwrap(),
        None
    );
    assert_eq!(
        p([huge, 0., 0.])
            .distance_bound_to(p([huge, 0., tiny]), tiny)
            .unwrap(),
        Some(tiny)
    );
    for limit in [-1., f64::INFINITY, f64::NAN] {
        assert!(zero.distance_bound_to(zero, limit).is_err());
    }
}
