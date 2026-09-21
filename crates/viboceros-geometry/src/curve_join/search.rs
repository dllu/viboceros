//! Conservative endpoint broad phase, independent of chain and curve policy.
//!
//! Positive radii use a balanced bounding-box tree. Pruning compares the same
//! rounded coordinate differences as the final distance test: no translated
//! grid coordinates, division by a radius, or expanded world coordinates are
//! needed. The boxes contain original finite endpoint coordinates exactly.

use super::{
    Candidate, CurveJoinOptions, Endpoint, GeometryError, MAX_JOIN_CANDIDATES, MAX_JOIN_SCANS, Real,
};
use std::collections::HashMap;

const LEAF_SIZE: usize = 8;

pub(super) fn find_candidates(
    endpoints: &[Endpoint],
    options: CurveJoinOptions,
) -> Result<Vec<Candidate>, GeometryError> {
    let mut candidates = Vec::new();
    let mut scans = 0;
    let mut consider = |left: usize, right: usize| -> Result<(), GeometryError> {
        scans += 1;
        if scans > MAX_JOIN_SCANS {
            return Err(limit("endpoint comparisons"));
        }
        if let Some(candidate) = candidate(endpoints, left, right, options)? {
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
        let tree = EndpointTree::new(endpoints);
        tree.visit_pairs(0, 0, options.tolerance, &mut 0, &mut consider)?;
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

#[derive(Clone, Copy)]
struct Node {
    minimum: [Real; 3],
    maximum: [Real; 3],
    start: usize,
    end: usize,
    children: Option<[usize; 2]>,
}

struct EndpointTree {
    indices: Vec<usize>,
    nodes: Vec<Node>,
}

impl EndpointTree {
    fn new(endpoints: &[Endpoint]) -> Self {
        let mut tree = Self {
            indices: (0..endpoints.len()).collect(),
            nodes: Vec::new(),
        };
        build(endpoints, &mut tree.indices, 0, &mut tree.nodes);
        tree
    }

    fn visit_pairs(
        &self,
        left: usize,
        right: usize,
        radius: Real,
        checks: &mut usize,
        consider: &mut impl FnMut(usize, usize) -> Result<(), GeometryError>,
    ) -> Result<(), GeometryError> {
        *checks += 1;
        if *checks > MAX_JOIN_SCANS {
            return Err(limit("endpoint bounding-box comparisons"));
        }
        let a = self.nodes[left];
        let b = self.nodes[right];
        if left != right && separated(a, b, radius) {
            return Ok(());
        }
        if left == right {
            if let Some([low, high]) = a.children {
                // Disjoint partitions cover every unordered pair exactly once.
                self.visit_pairs(low, low, radius, checks, consider)?;
                self.visit_pairs(low, high, radius, checks, consider)?;
                self.visit_pairs(high, high, radius, checks, consider)?;
            } else {
                for i in a.start..a.end {
                    for j in a.start..i {
                        consider(self.indices[j], self.indices[i])?;
                    }
                }
            }
        } else if a.children.is_some()
            && (b.children.is_none() || a.end - a.start >= b.end - b.start)
        {
            for child in a.children.expect("internal node") {
                self.visit_pairs(child, right, radius, checks, consider)?;
            }
        } else if let Some(children) = b.children {
            for child in children {
                self.visit_pairs(left, child, radius, checks, consider)?;
            }
        } else {
            for &i in &self.indices[a.start..a.end] {
                for &j in &self.indices[b.start..b.end] {
                    consider(i, j)?;
                }
            }
        }
        Ok(())
    }
}

fn separated(a: Node, b: Node, radius: Real) -> bool {
    // Subtraction is monotone. Each positive interval gap is no larger than
    // the corresponding rounded endpoint difference for any pair in the
    // boxes. Overflow means every such pair also exceeds the finite radius.
    (0..3).any(|axis| {
        a.minimum[axis] - b.maximum[axis] > radius || b.minimum[axis] - a.maximum[axis] > radius
    })
}

fn build(
    endpoints: &[Endpoint],
    indices: &mut [usize],
    start: usize,
    nodes: &mut Vec<Node>,
) -> usize {
    let mut minimum = [Real::INFINITY; 3];
    let mut maximum = [Real::NEG_INFINITY; 3];
    for &index in indices.iter() {
        let coordinates = endpoints[index].point.to_array();
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(coordinates[axis]);
            maximum[axis] = maximum[axis].max(coordinates[axis]);
        }
    }
    let root = nodes.len();
    nodes.push(Node {
        minimum,
        maximum,
        start,
        end: start + indices.len(),
        children: None,
    });
    if indices.len() > LEAF_SIZE {
        let axis = (0..3)
            .max_by(|&a, &b| (maximum[a] - minimum[a]).total_cmp(&(maximum[b] - minimum[b])))
            .expect("three axes");
        let middle = indices.len() / 2;
        indices.select_nth_unstable_by(middle, |&a, &b| {
            endpoints[a].point.to_array()[axis]
                .total_cmp(&endpoints[b].point.to_array()[axis])
                .then_with(|| a.cmp(&b))
        });
        let (low, high) = indices.split_at_mut(middle);
        nodes[root].children = Some([
            build(endpoints, low, start, nodes),
            build(endpoints, high, start + middle, nodes),
        ]);
    }
    root
}
