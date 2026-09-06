//! Trim boundaries plus stationary-point search over original knot patches.
use super::{
    bezier::{self, Budget, Net},
    parameter_curves::Prepared,
    trim_region::{Location, Rect, Region},
};
use crate::{BoundingBox3, Brep, BrepFace, GeometryError, Point3, Tolerance};

impl BrepFace {
    /// Tolerance-controlled bounds of the entire trimmed surface face,
    /// including holes, interior extrema, seams, and singular boundaries.
    /// Requires closed, continuous UV loops; does not repair trim gaps or use
    /// stored 3D edge approximations. Ambiguity, poles, and exhausted budgets
    /// return errors. Floating-point refinement is not interval certification.
    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        face_bounds(self, &mut Budget::default(), tolerance)
    }
}

impl Brep {
    /// Tight bounds of all trimmed faces using one shared resource budget.
    /// Unused topology and approximate shared edges do not enlarge the box.
    pub fn tight_bounds(&self, tolerance: Tolerance) -> Result<BoundingBox3, GeometryError> {
        let mut budget = Budget::default();
        let mut bounds = None;
        for face in self.faces() {
            bezier::merge(&mut bounds, face_bounds(face, &mut budget, tolerance)?)?;
        }
        bounds.ok_or(GeometryError::EmptyPointSet)
    }
}

struct Node {
    net: Net,
    domain: Rect,
    // Only ORIGINAL patch edges may carry nonstationary extrema. Artificial
    // subdivision edges are not boundaries of the trimmed surface.
    sides: [[bool; 2]; 2],
}

fn face_bounds(
    face: &BrepFace,
    budget: &mut Budget,
    tolerance: Tolerance,
) -> Result<BoundingBox3, GeometryError> {
    let prepared = Prepared::new(face.surface(), budget)?;
    let mut region = Region::new(face, prepared.domains, budget)?;
    let boundary = prepared.boundary_bounds(face, budget, tolerance)?;
    let mut attained = boundary.attained;
    let mut enclosure = boundary.enclosure;
    let mut pending = prepared
        .patches
        .into_iter()
        .map(|p| Node {
            net: p.net,
            domain: p.domain,
            sides: [[true; 2]; 2],
        })
        .collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        budget.visit()?;
        let hull = node.net.hull();
        let signs = if hull.is_some() {
            node.net.derivative_signs(budget)?
        } else {
            [[0; 2]; 3]
        };
        let relevant = |axis: usize, side: usize| {
            (0..2).all(|direction| {
                let sign = signs[axis][direction];
                sign == 0 || node.sides[direction][if sign > 0 { side } else { 1 - side }]
            })
        };
        let restrict = |h: BoundingBox3, a: BoundingBox3| -> Result<BoundingBox3, GeometryError> {
            let low = std::array::from_fn(|i| {
                if relevant(i, 0) {
                    h.min().to_array()[i].min(a.min().to_array()[i])
                } else {
                    a.min().to_array()[i]
                }
            });
            let high = std::array::from_fn(|i| {
                if relevant(i, 1) {
                    h.max().to_array()[i].max(a.max().to_array()[i])
                } else {
                    a.max().to_array()[i]
                }
            });
            BoundingBox3::from_points([Point3::try_from(low)?, Point3::try_from(high)?])
        };
        if let Some(h) = hull {
            let candidate = restrict(h, attained)?;
            if bezier::resolved(candidate, attained, tolerance) {
                enclosure = enclosure.union(candidate)?;
                continue;
            }
        }
        let location = region.locate(node.domain, budget)?;
        if location == Location::Outside {
            continue;
        }
        let [p, q] = node.net.degrees;
        let work = node
            .net
            .controls
            .len()
            .saturating_mul(p.saturating_add(q).saturating_add(1));
        budget.charge(work)?;
        let center = node.domain.map(|d| d[0] * 0.5 + d[1] * 0.5);
        let candidates = [
            ([node.domain[0][0], node.domain[1][0]], Some(0)),
            ([node.domain[0][1], node.domain[1][0]], Some(p)),
            ([node.domain[0][0], node.domain[1][1]], Some(q * (p + 1))),
            (
                [node.domain[0][1], node.domain[1][1]],
                Some((p + 1) * (q + 1) - 1),
            ),
            (center, None),
        ];
        for (uv, index) in candidates {
            if location == Location::Inside
                || region.locate(uv.map(|x| [x, x]), budget)? == Location::Inside
            {
                // Evaluate the homogeneous patch, not rounded native UVs.
                let point = if let Some(i) = index {
                    node.net.project(node.net.controls[i])?
                } else {
                    node.net.center()?
                };
                attained = attained.union(BoundingBox3::from_points([point])?)?;
            }
        }
        if let Some(h) = hull {
            let candidate = restrict(h, attained)?;
            if bezier::resolved(candidate, attained, tolerance) {
                enclosure = enclosure.union(candidate)?;
                continue;
            }
        }
        if node.net.depth == bezier::MAX_DEPTH {
            return Err(GeometryError::BoundingBoxDidNotConverge);
        }
        budget.charge(work)?;
        let axis = node.net.split_axis(hull, attained, tolerance);
        let middle = center[axis];
        if middle <= node.domain[axis][0] || middle >= node.domain[axis][1] {
            return Err(GeometryError::BoundingBoxDidNotConverge);
        }
        let (left, right) = node.net.split(axis);
        let mut a = Node {
            net: left,
            domain: node.domain,
            sides: node.sides,
        };
        let mut b = Node {
            net: right,
            domain: node.domain,
            sides: node.sides,
        };
        a.domain[axis][1] = middle;
        a.sides[axis][1] = false;
        b.domain[axis][0] = middle;
        b.sides[axis][0] = false;
        pending.push(b);
        pending.push(a);
    }
    enclosure.union(attained)
}

#[cfg(test)]
pub(super) mod tests;
