//! Deterministic axis-aligned k-d tree construction and bounded queries.

use super::{Point3, PointCloudProjection, Real};
use std::cmp::Ordering;

#[derive(Debug)]
pub(super) struct ProjectedIndex {
    pub(super) nodes: Vec<ProjectedNode>,
    pub(super) root: usize,
    axes: [u8; 3],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProjectedNode {
    point_index: usize,
    axis: u8,
    left: Option<usize>,
    right: Option<usize>,
}

impl ProjectedIndex {
    pub(super) fn new(points: &[Point3], projection: PointCloudProjection) -> Self {
        let mut indices = (0..points.len()).collect::<Vec<_>>();
        let mut nodes = Vec::with_capacity(points.len());
        let axes = projection.axes();
        let root = build_projected_tree(points, &mut indices, 0, &mut nodes, axes);
        Self { nodes, root, axes }
    }

    pub(super) fn nearest_from(
        &self,
        node_index: usize,
        points: &[Point3],
        origin: Point3,
        offset: [Real; 2],
        maximum_distance: Real,
        best: &mut Option<(Real, usize)>,
    ) {
        let node = self.nodes[node_index];
        let point = points[node.point_index];
        let relative = [
            (coordinate(point, self.axes[0]) - coordinate(origin, self.axes[0])) - offset[0],
            (coordinate(point, self.axes[1]) - coordinate(origin, self.axes[1])) - offset[1],
        ];
        let distance = relative[0].hypot(relative[1]);
        if distance <= maximum_distance
            && best.is_none_or(|(best_distance, best_index)| {
                distance < best_distance
                    || (distance == best_distance && node.point_index < best_index)
            })
        {
            *best = Some((distance, node.point_index));
        }

        let delta = -relative[usize::from(node.axis)];
        let (near, far) = if delta < 0.0 {
            (node.left, node.right)
        } else {
            (node.right, node.left)
        };
        if let Some(near) = near {
            self.nearest_from(near, points, origin, offset, maximum_distance, best);
        }
        let search_distance = best.map_or(maximum_distance, |(distance, _)| distance);
        if delta.abs() <= search_distance
            && let Some(far) = far
        {
            self.nearest_from(far, points, origin, offset, maximum_distance, best);
        }
    }
}

fn build_projected_tree(
    points: &[Point3],
    point_indices: &mut [usize],
    depth: usize,
    nodes: &mut Vec<ProjectedNode>,
    axes: [u8; 3],
) -> usize {
    debug_assert!(!point_indices.is_empty());
    let axis = (depth % 2) as u8;
    let middle = point_indices.len() / 2;
    point_indices.select_nth_unstable_by(middle, |left, right| {
        compare_point_indices(points, *left, *right, axis, axes)
    });
    let (left_indices, middle_and_right) = point_indices.split_at_mut(middle);
    let (middle_index, right_indices) = middle_and_right
        .split_first_mut()
        .expect("a nonempty slice has a middle element");
    let node_index = nodes.len();
    nodes.push(ProjectedNode {
        point_index: *middle_index,
        axis,
        left: None,
        right: None,
    });
    let left = (!left_indices.is_empty())
        .then(|| build_projected_tree(points, left_indices, depth + 1, nodes, axes));
    let right = (!right_indices.is_empty())
        .then(|| build_projected_tree(points, right_indices, depth + 1, nodes, axes));
    nodes[node_index].left = left;
    nodes[node_index].right = right;
    node_index
}

fn compare_point_indices(
    points: &[Point3],
    left: usize,
    right: usize,
    axis: u8,
    axes: [u8; 3],
) -> Ordering {
    coordinate(points[left], axes[usize::from(axis)])
        .total_cmp(&coordinate(points[right], axes[usize::from(axis)]))
        .then_with(|| {
            coordinate(points[left], axes[usize::from(axis ^ 1)])
                .total_cmp(&coordinate(points[right], axes[usize::from(axis ^ 1)]))
        })
        .then_with(|| {
            coordinate(points[left], axes[2]).total_cmp(&coordinate(points[right], axes[2]))
        })
        .then_with(|| left.cmp(&right))
}

#[inline]
fn coordinate(point: Point3, axis: u8) -> Real {
    point.to_array()[usize::from(axis)]
}
