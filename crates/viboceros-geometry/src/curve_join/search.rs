//! Conservative endpoint broad phase, independent of chain and curve policy.
//!
//! Positive radii use a balanced bounding-box tree. Pruning compares the same
//! rounded coordinate differences as the final distance test: no translated
//! grid coordinates, division by a radius, or expanded world coordinates are
//! needed. The boxes contain original finite endpoint coordinates exactly.

use super::{
    Candidate, CurveJoinOptions, Endpoint, GeometryError, MAX_JOIN_CANDIDATES, MAX_JOIN_SCANS,
};
use std::collections::HashMap;

use crate::point_neighbors::PointTree;

#[cfg(test)]
pub(super) fn find_candidates(
    endpoints: &[Endpoint],
    options: CurveJoinOptions,
) -> Result<Vec<Candidate>, GeometryError> {
    find_candidates_with_filter(endpoints, options, &mut |_, _| true)
}

pub(super) fn find_candidates_with_filter(
    endpoints: &[Endpoint],
    options: CurveJoinOptions,
    allow: &mut impl FnMut(usize, usize) -> bool,
) -> Result<Vec<Candidate>, GeometryError> {
    let mut candidates = Vec::new();
    let mut scans = 0;
    let mut consider = |left: usize, right: usize| -> Result<(), GeometryError> {
        scans += 1;
        if scans > MAX_JOIN_SCANS {
            return Err(limit("endpoint comparisons"));
        }
        if allow(left, right)
            && let Some(candidate) = candidate(endpoints, left, right, options)?
        {
            if candidates.len() == MAX_JOIN_CANDIDATES {
                return Err(GeometryError::CurveJoinLimit {
                    resource: "endpoint candidates",
                    maximum: MAX_JOIN_CANDIDATES,
                });
            }
            candidates.push(candidate);
        }
        Ok(())
    };
    if options.tolerance == 0.0 {
        let mut exact = HashMap::<[u64; 3], Vec<usize>>::new();
        for (index, endpoint) in endpoints.iter().enumerate() {
            let key = endpoint
                .point
                .to_array()
                .map(|value| if value == 0.0 { 0 } else { value.to_bits() });
            let bucket = exact.entry(key).or_default();
            for &other in bucket.iter() {
                consider(other, index)?;
            }
            bucket.push(index);
        }
    } else if endpoints.len() > 1 {
        let tree = PointTree::new(endpoints.iter().map(|e| e.point).collect());
        let mut checks = 0;
        let mut check = || {
            checks += 1;
            if checks > MAX_JOIN_SCANS {
                Err(limit("endpoint bounding-box comparisons"))
            } else {
                Ok(())
            }
        };
        tree.visit_pairs(options.tolerance, &mut check, &mut consider)?;
    }
    Ok(candidates)
}

/// Shared narrow-phase predicate and rank for batch and individual-pick joins.
pub(super) fn candidate(
    endpoints: &[Endpoint],
    left: usize,
    right: usize,
    options: CurveJoinOptions,
) -> Result<Option<Candidate>, GeometryError> {
    let a = endpoints[left];
    let b = endpoints[right];
    if a.curve == b.curve || (options.preserve_direction && a.start == b.start) {
        return Ok(None);
    }
    let distance = (a.point.x() - b.point.x())
        .hypot(a.point.y() - b.point.y())
        .hypot(a.point.z() - b.point.z());
    if distance > options.tolerance {
        return Ok(None);
    }
    let tangent_dot = match (a.outward_tangent, b.outward_tangent) {
        (Some(a), Some(b)) => a.as_vector().dot(b.as_vector())?,
        _ => 1.0,
    };
    Ok(Some(Candidate {
        distance,
        tangent_dot,
        left: left.min(right),
        right: left.max(right),
    }))
}

fn limit(resource: &'static str) -> GeometryError {
    GeometryError::CurveJoinLimit {
        resource,
        maximum: MAX_JOIN_SCANS,
    }
}
