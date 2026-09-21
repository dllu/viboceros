//! Snapshot-cached mesh wires with camera-independent bounds hierarchy.
use super::{ObjectSnapKind, ObjectSnapModes, SnapMetric, near, projected_line, proximity};
use std::collections::{BTreeMap, HashMap};
use viboceros_document::{Geometry, GeometrySnapshot, Object, ObjectId};
use viboceros_geometry::{BoundingBox3, Point3, Real};

#[derive(Debug)]
struct Wire {
    order: usize,
    ends: [Point3; 2],
    midpoint: Point3,
    bounds: BoundingBox3,
}

#[derive(Debug)]
struct Node {
    bounds: BoundingBox3,
    range: std::ops::Range<usize>,
    children: Option<[usize; 2]>,
}

#[derive(Debug)]
struct Index {
    source: GeometrySnapshot,
    wires: Vec<Wire>,
    nodes: Vec<Node>,
}

impl Index {
    fn new(object: &Object) -> Self {
        let Geometry::Mesh(mesh) = object.geometry() else {
            unreachable!()
        };
        let mut index = Self {
            source: object.geometry_snapshot().clone(),
            wires: mesh
                .topology_edge_points()
                .into_iter()
                .enumerate()
                .filter(|(_, ends)| ends[0] != ends[1])
                .map(|(order, ends)| Wire {
                    order,
                    midpoint: ends[0].midpoint(ends[1]).expect("finite endpoints"),
                    bounds: BoundingBox3::from_points(ends).expect("finite endpoints"),
                    ends,
                })
                .collect(),
            nodes: Vec::new(),
        };
        if !index.wires.is_empty() {
            index.build(0..index.wires.len());
        }
        index
    }

    fn build(&mut self, range: std::ops::Range<usize>) -> usize {
        let bounds = self.wires[range.clone()]
            .iter()
            .skip(1)
            .fold(self.wires[range.start].bounds, |a, b| {
                a.union(b.bounds).expect("finite bounds")
            });
        let node = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 8 {
            let lo = bounds.min().to_array();
            let hi = bounds.max().to_array();
            let axis = (0..3)
                .max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
                .unwrap();
            let middle = range.start + range.len() / 2;
            self.wires[range.clone()].select_nth_unstable_by(range.len() / 2, |a, b| {
                a.midpoint.to_array()[axis].total_cmp(&b.midpoint.to_array()[axis])
            });
            let children = [
                self.build(range.start..middle),
                self.build(middle..range.end),
            ];
            self.nodes[node].children = Some(children);
        }
        node
    }

    fn visit(&self, node: usize, metric: &impl SnapMetric, visitor: &mut impl FnMut(&Wire)) {
        let node = &self.nodes[node];
        if proximity::outside_bounds(
            node.bounds.min().to_array(),
            node.bounds.max().to_array(),
            metric,
        ) {
            return;
        }
        if let Some(children) = node.children {
            for child in children {
                self.visit(child, metric, visitor);
            }
        } else {
            for wire in &self.wires[node.range.clone()] {
                visitor(wire);
            }
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Cache {
    entries: BTreeMap<ObjectId, Index>,
    #[cfg(test)]
    builds: usize,
    #[cfg(test)]
    visited: usize,
}

impl Cache {
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn retain_objects(&mut self, live: &HashMap<ObjectId, &Geometry>) {
        self.entries
            .retain(|id, _| live.get(id).is_some_and(|g| matches!(g, Geometry::Mesh(_))));
    }

    pub(super) fn visit(
        &mut self,
        object: &Object,
        modes: ObjectSnapModes,
        metric: &impl SnapMetric,
        emit: &mut impl FnMut(ObjectSnapKind, Point3, Real),
    ) {
        let mid = modes.contains(ObjectSnapKind::Mid);
        let near = modes.contains(ObjectSnapKind::Near);
        if !mid && !near {
            return;
        }
        let index = self.entries.entry(object.id()).or_insert_with(|| {
            #[cfg(test)]
            {
                self.builds += 1;
            }
            Index::new(object)
        });
        if !index.source.shares_storage_with(object.geometry_snapshot()) {
            *index = Index::new(object);
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
        if index.nodes.is_empty() {
            return;
        }
        let mut best_mid = None;
        let mut best_near = None;
        let mut consider = |wire: &Wire| {
            #[cfg(test)]
            {
                self.visited += 1;
            }
            if mid && let Some(direct) = metric.captured_distance(wire.midpoint) {
                // Retained Rhino mesh picks require proximity to the midpoint
                // itself, including Mid-only and one-shot Mid. Curve whole-
                // segment hover must not be generalized to mesh wires.
                keep(&mut best_mid, wire.order, wire.midpoint, direct);
            }
            if near && best_mid.is_none() {
                let mut emit = |point, distance| {
                    keep(&mut best_near, wire.order, point, distance);
                };
                line(wire.ends[0], wire.ends[1], metric, &mut emit);
            }
        };
        index.visit(0, metric, &mut consider);
        if let Some((_, point, distance)) = best_mid {
            emit(ObjectSnapKind::Mid, point, distance);
        } else if let Some((_, point, distance)) = best_near {
            emit(ObjectSnapKind::Near, point, distance);
        }
    }
}

fn keep(best: &mut Option<(usize, Point3, Real)>, order: usize, point: Point3, distance: Real) {
    if best.is_none_or(|(old_order, _, score)| {
        distance < score || (distance == score && order < old_order)
    }) {
        *best = Some((order, point, distance));
    }
}

fn line(a: Point3, b: Point3, metric: &impl SnapMetric, emit: &mut impl FnMut(Point3, Real)) {
    match projected_line::capture_mesh(a, b, metric) {
        projected_line::Capture::Point(point) => {
            if let Some(distance) = metric.captured_distance(point) {
                emit(point, distance);
            }
        }
        projected_line::Capture::Miss => {}
        projected_line::Capture::Unresolved => near::line(a, b, metric, emit),
    }
}

#[cfg(test)]
mod tests;
