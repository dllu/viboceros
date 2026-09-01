use std::cmp::Ordering;

use crate::{AffineTransform3, BoundingBox3, GeometryError, Point3, Real};

/// An immutable, finite collection of 3D points.
///
/// The stored order is significant, matching Rhino point clouds and 3DM
/// archives. A balanced XY k-d tree is built once so top-view picking and
/// object snapping do not require a full scan on every pointer update.
#[derive(Clone, Debug)]
pub struct PointCloud3 {
    points: Vec<Point3>,
    bounds: BoundingBox3,
    xy_nodes: Vec<XyNode>,
    xy_root: usize,
}

#[derive(Clone, Copy, Debug)]
struct XyNode {
    point_index: usize,
    axis: u8,
    left: Option<usize>,
    right: Option<usize>,
}

impl PointCloud3 {
    /// Creates a point cloud while preserving point order and duplicates.
    /// Empty point clouds are rejected because OpenNURBS considers them
    /// invalid and they have no finite bounding box.
    pub fn try_new(points: Vec<Point3>) -> Result<Self, GeometryError> {
        let bounds = BoundingBox3::from_points(points.iter().copied())?;
        let mut point_indices = (0..points.len()).collect::<Vec<_>>();
        let mut xy_nodes = Vec::with_capacity(points.len());
        let xy_root = build_xy_tree(&points, &mut point_indices, 0, &mut xy_nodes);
        Ok(Self {
            points,
            bounds,
            xy_nodes,
            xy_root,
        })
    }

    #[inline]
    pub fn points(&self) -> &[Point3] {
        &self.points
    }

    #[inline]
    pub const fn bounds(&self) -> BoundingBox3 {
        self.bounds
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        Self::try_new(
            self.points
                .iter()
                .map(|point| transform.transform_point(*point))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }

    /// Returns the nearest point in the XY projection within `maximum_distance`.
    /// Exact-distance ties use the earliest stored point.
    pub fn nearest_xy(
        &self,
        query: Point3,
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        if !maximum_distance.is_finite() || maximum_distance < 0.0 {
            return Err(GeometryError::InvalidPointCloudSearchRadius);
        }
        let mut best = None;
        self.nearest_xy_from(self.xy_root, query, maximum_distance, &mut best);
        Ok(best.map(|(distance, point_index)| (point_index, self.points[point_index], distance)))
    }

    fn nearest_xy_from(
        &self,
        node_index: usize,
        query: Point3,
        maximum_distance: Real,
        best: &mut Option<(Real, usize)>,
    ) {
        let node = self.xy_nodes[node_index];
        let point = self.points[node.point_index];
        let distance = (point.x() - query.x()).hypot(point.y() - query.y());
        if distance <= maximum_distance
            && best.is_none_or(|(best_distance, best_index)| {
                distance < best_distance
                    || (distance == best_distance && node.point_index < best_index)
            })
        {
            *best = Some((distance, node.point_index));
        }

        let delta = coordinate(query, node.axis) - coordinate(point, node.axis);
        let (near, far) = if delta < 0.0 {
            (node.left, node.right)
        } else {
            (node.right, node.left)
        };
        if let Some(near) = near {
            self.nearest_xy_from(near, query, maximum_distance, best);
        }
        let search_distance = best.map_or(maximum_distance, |(distance, _)| distance);
        if delta.abs() <= search_distance
            && let Some(far) = far
        {
            self.nearest_xy_from(far, query, maximum_distance, best);
        }
    }
}

impl PartialEq for PointCloud3 {
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
    }
}

fn build_xy_tree(
    points: &[Point3],
    point_indices: &mut [usize],
    depth: usize,
    nodes: &mut Vec<XyNode>,
) -> usize {
    debug_assert!(!point_indices.is_empty());
    let axis = (depth % 2) as u8;
    let middle = point_indices.len() / 2;
    point_indices.select_nth_unstable_by(middle, |left, right| {
        compare_point_indices(points, *left, *right, axis)
    });
    let (left_indices, middle_and_right) = point_indices.split_at_mut(middle);
    let (middle_index, right_indices) = middle_and_right
        .split_first_mut()
        .expect("a nonempty slice has a middle element");
    let node_index = nodes.len();
    nodes.push(XyNode {
        point_index: *middle_index,
        axis,
        left: None,
        right: None,
    });
    let left =
        (!left_indices.is_empty()).then(|| build_xy_tree(points, left_indices, depth + 1, nodes));
    let right =
        (!right_indices.is_empty()).then(|| build_xy_tree(points, right_indices, depth + 1, nodes));
    nodes[node_index].left = left;
    nodes[node_index].right = right;
    node_index
}

fn compare_point_indices(points: &[Point3], left: usize, right: usize, axis: u8) -> Ordering {
    coordinate(points[left], axis)
        .total_cmp(&coordinate(points[right], axis))
        .then_with(|| {
            coordinate(points[left], axis ^ 1).total_cmp(&coordinate(points[right], axis ^ 1))
        })
        .then_with(|| points[left].z().total_cmp(&points[right].z()))
        .then_with(|| left.cmp(&right))
}

#[inline]
fn coordinate(point: Point3, axis: u8) -> Real {
    if axis == 0 { point.x() } else { point.y() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn requires_points_and_preserves_order_duplicates_and_bounds() {
        assert_eq!(
            PointCloud3::try_new(Vec::new()),
            Err(GeometryError::EmptyPointSet)
        );
        let points = vec![
            point(3.0, -2.0, 4.0),
            point(-1.0, 5.0, 0.0),
            point(3.0, -2.0, 4.0),
        ];
        let cloud = PointCloud3::try_new(points.clone()).unwrap();
        assert_eq!(cloud.points(), points);
        assert_eq!(cloud.bounds().min(), point(-1.0, -2.0, 0.0));
        assert_eq!(cloud.bounds().max(), point(3.0, 5.0, 4.0));
    }

    #[test]
    fn nearest_xy_is_inclusive_and_breaks_ties_by_source_order() {
        let cloud = PointCloud3::try_new(vec![
            point(-1.0, 0.0, 9.0),
            point(1.0, 0.0, -9.0),
            point(0.0, 4.0, 0.0),
        ])
        .unwrap();
        assert_eq!(
            cloud.nearest_xy(point(0.0, 0.0, 100.0), 1.0).unwrap(),
            Some((0, point(-1.0, 0.0, 9.0), 1.0))
        );
        assert_eq!(cloud.nearest_xy(point(0.0, 0.0, 0.0), 0.999).unwrap(), None);
        assert_eq!(
            cloud.nearest_xy(point(0.0, 0.0, 0.0), -1.0),
            Err(GeometryError::InvalidPointCloudSearchRadius)
        );
    }

    #[test]
    fn transforms_every_point_and_rebuilds_the_spatial_index() {
        let cloud = PointCloud3::try_new(vec![point(0.0, 0.0, 0.0), point(2.0, 3.0, 4.0)]).unwrap();
        let transformed = cloud
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(10.0, -2.0, 1.0).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            transformed.points(),
            [point(10.0, -2.0, 1.0), point(12.0, 1.0, 5.0)]
        );
        assert_eq!(
            transformed
                .nearest_xy(point(12.0, 1.0, 99.0), 0.0)
                .unwrap()
                .map(|(index, point, _)| (index, point)),
            Some((1, point(12.0, 1.0, 5.0)))
        );
    }

    #[test]
    fn indexed_nearest_matches_a_stable_brute_force_search() {
        fn random_unit(state: &mut u64) -> Real {
            *state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (*state >> 11) as Real / ((1_u64 << 53) as Real)
        }

        let mut state = 0x9e37_79b9_7f4a_7c15;
        let points = (0..513)
            .map(|index| {
                if index % 37 == 0 {
                    point(2.0, -3.0, index as Real)
                } else {
                    point(
                        random_unit(&mut state) * 200.0 - 100.0,
                        random_unit(&mut state) * 200.0 - 100.0,
                        random_unit(&mut state) * 20.0 - 10.0,
                    )
                }
            })
            .collect::<Vec<_>>();
        let cloud = PointCloud3::try_new(points.clone()).unwrap();

        for _ in 0..256 {
            let query = point(
                random_unit(&mut state) * 240.0 - 120.0,
                random_unit(&mut state) * 240.0 - 120.0,
                random_unit(&mut state) * 20.0 - 10.0,
            );
            let maximum_distance = random_unit(&mut state) * 80.0;
            let expected = points
                .iter()
                .enumerate()
                .filter_map(|(index, candidate)| {
                    let distance = (candidate.x() - query.x()).hypot(candidate.y() - query.y());
                    (distance <= maximum_distance).then_some((distance, index, *candidate))
                })
                .min_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                })
                .map(|(distance, index, candidate)| (index, candidate, distance));
            assert_eq!(cloud.nearest_xy(query, maximum_distance).unwrap(), expected);
        }

        assert_eq!(
            cloud.nearest_xy(point(0.0, 0.0, 0.0), Real::INFINITY),
            Err(GeometryError::InvalidPointCloudSearchRadius)
        );
    }
}
