use std::cmp::Ordering;
use std::sync::{Arc, OnceLock};

use crate::{AffineTransform3, BoundingBox3, GeometryError, Point3, Real};

/// Axis-aligned projection used by a point-cloud spatial query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointCloudProjection {
    Xy,
    Xz,
    Yz,
}

impl PointCloudProjection {
    const fn axes(self) -> [u8; 3] {
        match self {
            Self::Xy => [0, 1, 2],
            Self::Xz => [0, 2, 1],
            Self::Yz => [1, 2, 0],
        }
    }
}

#[derive(Debug)]
struct ProjectedIndex {
    nodes: Vec<ProjectedNode>,
    root: usize,
    axes: [u8; 3],
}

/// An immutable, finite collection of 3D points.
///
/// The stored order is significant, matching Rhino point clouds and 3DM
/// archives. A balanced XY k-d tree is built immediately; XZ/YZ indexes are
/// initialized on demand and reused for axis-aligned viewport queries.
/// Clones share immutable point storage and all projection caches; transforms
/// construct a new data block rather than modifying shared geometry.
#[derive(Clone, Debug)]
pub struct PointCloud3 {
    data: Arc<PointCloudData>,
}

#[derive(Debug)]
struct PointCloudData {
    points: Vec<Point3>,
    bounds: BoundingBox3,
    xy: ProjectedIndex,
    xz: OnceLock<ProjectedIndex>,
    yz: OnceLock<ProjectedIndex>,
}

#[derive(Clone, Copy, Debug)]
struct ProjectedNode {
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
        let xy = ProjectedIndex::new(&points, PointCloudProjection::Xy);
        Ok(Self {
            data: Arc::new(PointCloudData {
                points,
                bounds,
                xy,
                xz: OnceLock::new(),
                yz: OnceLock::new(),
            }),
        })
    }

    #[inline]
    pub fn points(&self) -> &[Point3] {
        &self.data.points
    }

    #[inline]
    pub fn bounds(&self) -> BoundingBox3 {
        self.data.bounds
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        Self::try_new(
            self.data
                .points
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
        self.nearest_xy_relative(query, [0.0; 2], maximum_distance)
    }

    /// Searches XY distances evaluated as `(point - origin) - offset`.
    /// Keeping the local offset separate avoids rounding it away when the
    /// origin has large absolute coordinates. Ties retain source order.
    pub fn nearest_xy_relative(
        &self,
        origin: Point3,
        offset: [Real; 2],
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        self.nearest_projected_relative(PointCloudProjection::Xy, origin, offset, maximum_distance)
    }

    /// Searches `(project(point) - project(origin)) - offset` without forming
    /// an absolute cursor. XZ/YZ indexes are initialized once, on first valid
    /// use; XY is available immediately. Exact ties use stored point order.
    pub fn nearest_projected_relative(
        &self,
        projection: PointCloudProjection,
        origin: Point3,
        offset: [Real; 2],
        maximum_distance: Real,
    ) -> Result<Option<(usize, Point3, Real)>, GeometryError> {
        if !maximum_distance.is_finite() || maximum_distance < 0.0 {
            return Err(GeometryError::InvalidPointCloudSearchRadius);
        }
        if offset.iter().any(|value| !value.is_finite()) {
            return Err(GeometryError::NonFinite {
                context: "point cloud search offset",
            });
        }
        let index = match projection {
            PointCloudProjection::Xy => &self.data.xy,
            PointCloudProjection::Xz => self
                .data
                .xz
                .get_or_init(|| ProjectedIndex::new(&self.data.points, projection)),
            PointCloudProjection::Yz => self
                .data
                .yz
                .get_or_init(|| ProjectedIndex::new(&self.data.points, projection)),
        };
        let mut best = None;
        index.nearest_from(
            index.root,
            &self.data.points,
            origin,
            offset,
            maximum_distance,
            &mut best,
        );
        Ok(best
            .map(|(distance, point_index)| (point_index, self.data.points[point_index], distance)))
    }
}

impl ProjectedIndex {
    fn new(points: &[Point3], projection: PointCloudProjection) -> Self {
        let mut indices = (0..points.len()).collect::<Vec<_>>();
        let mut nodes = Vec::with_capacity(points.len());
        let axes = projection.axes();
        let root = build_projected_tree(points, &mut indices, 0, &mut nodes, axes);
        Self { nodes, root, axes }
    }

    fn nearest_from(
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

impl PartialEq for PointCloud3 {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) || self.data.points == other.data.points
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn projected_indexes_match_exhaustive_queries_in_each_plane() {
        for translation in [0.0, 2.0_f64.powi(52), -2.0_f64.powi(52)] {
            let origin = point(translation, translation, translation);
            let points: Vec<_> = (0..137)
                .map(|i| {
                    point(
                        translation + Real::from(i % 17 - 8),
                        translation + Real::from(i * 7 % 19 - 9),
                        translation + Real::from(i * 11 % 23 - 11),
                    )
                })
                .collect();
            let cloud = PointCloud3::try_new(points.clone()).unwrap();
            for (projection, axes) in [
                (PointCloudProjection::Xy, [0, 1]),
                (PointCloudProjection::Xz, [0, 2]),
                (PointCloudProjection::Yz, [1, 2]),
            ] {
                for i in 0..91 {
                    let offset = [
                        Real::from(i % 13 - 6) / 2.0,
                        Real::from(i * 5 % 17 - 8) / 2.0,
                    ];
                    for radius in [0.0, 0.5, 2.0, 100.0] {
                        let mut expected: Option<(usize, Point3, Real)> = None;
                        for (index, &p) in points.iter().enumerate() {
                            let coordinates = p.to_array();
                            let distance = ((coordinates[axes[0]] - translation) - offset[0])
                                .hypot((coordinates[axes[1]] - translation) - offset[1]);
                            if distance <= radius
                                && expected.is_none_or(|(_, _, best)| distance < best)
                            {
                                expected = Some((index, p, distance));
                            }
                        }
                        assert_eq!(
                            cloud
                                .nearest_projected_relative(projection, origin, offset, radius)
                                .unwrap(),
                            expected
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn additional_projection_indexes_are_lazy_and_reused() {
        let cloud = PointCloud3::try_new(vec![point(0.0, 1.0, 2.0), point(3.0, 4.0, 5.0)]).unwrap();
        assert!(cloud.data.xz.get().is_none() && cloud.data.yz.get().is_none());
        let origin = point(0.0, 0.0, 0.0);
        assert!(
            cloud
                .nearest_projected_relative(PointCloudProjection::Xz, origin, [Real::NAN, 0.0], 1.0)
                .is_err()
        );
        assert!(cloud.data.xz.get().is_none());
        cloud
            .nearest_projected_relative(PointCloudProjection::Xz, origin, [0.0; 2], 10.0)
            .unwrap();
        let allocation = cloud.data.xz.get().unwrap().nodes.as_ptr();
        assert!(cloud.data.yz.get().is_none());
        cloud
            .nearest_projected_relative(PointCloudProjection::Xz, origin, [1.0; 2], 10.0)
            .unwrap();
        assert_eq!(cloud.data.xz.get().unwrap().nodes.as_ptr(), allocation);
        assert_eq!(cloud.clone(), cloud);
    }

    #[test]
    fn cloned_cloud_queries_preserve_results_and_equality() {
        let cloud = PointCloud3::try_new(vec![point(0.0, 1.0, 2.0), point(3.0, 4.0, 5.0)]).unwrap();
        let cold_clone = cloud.clone();
        let query = point(3.0, 4.0, 5.0);
        cloud
            .nearest_projected_relative(PointCloudProjection::Xz, query, [0.0; 2], 0.0)
            .unwrap();
        let cloned = cloud.clone();
        assert_eq!(cloud, cloned);
        for projection in [
            PointCloudProjection::Xy,
            PointCloudProjection::Xz,
            PointCloudProjection::Yz,
        ] {
            for candidate in [&cloud, &cold_clone, &cloned] {
                assert_eq!(
                    candidate
                        .nearest_projected_relative(projection, query, [0.0; 2], 0.0)
                        .unwrap(),
                    Some((1, query, 0.0))
                );
            }
        }
        assert_eq!(cloud, cloned);
        assert_eq!(cloud, cold_clone);
    }

    #[test]
    fn clones_reuse_point_storage_and_lazy_projection_indexes() {
        let cloud = PointCloud3::try_new(vec![point(0.0, 1.0, 2.0), point(3.0, 4.0, 5.0)]).unwrap();
        let cloned = cloud.clone();
        assert_eq!(cloud.points().as_ptr(), cloned.points().as_ptr());
        assert!(cloud.data.xz.get().is_none());
        cloned
            .nearest_projected_relative(
                PointCloudProjection::Xz,
                point(3.0, 4.0, 5.0),
                [0.0; 2],
                0.0,
            )
            .unwrap();
        assert_eq!(
            cloud.data.xz.get().unwrap().nodes.as_ptr(),
            cloned.data.xz.get().unwrap().nodes.as_ptr()
        );
        drop(cloud);
        assert_eq!(
            cloned
                .nearest_projected_relative(
                    PointCloudProjection::Xz,
                    point(3.0, 4.0, 5.0),
                    [0.0; 2],
                    0.0
                )
                .unwrap(),
            Some((1, point(3.0, 4.0, 5.0), 0.0))
        );
    }

    #[test]
    fn concurrent_projection_queries_publish_and_reuse_one_index_per_plane() {
        let cloud = PointCloud3::try_new(
            (0..1024)
                .map(|i| point(Real::from(i), Real::from(i * 2), Real::from(i * 3)))
                .collect(),
        )
        .unwrap();
        let barrier = std::sync::Barrier::new(4);
        let allocations = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4)
                .map(|worker| {
                    let cloud = cloud.clone();
                    let barrier = &barrier;
                    scope.spawn(move || {
                        let projection = if worker % 2 == 0 {
                            PointCloudProjection::Xz
                        } else {
                            PointCloudProjection::Yz
                        };
                        barrier.wait();
                        for query in 0..128 {
                            let index = (query * 31 + worker) % 1024;
                            let p = cloud.data.points[index];
                            assert_eq!(
                                cloud
                                    .nearest_projected_relative(projection, p, [0.0; 2], 0.0)
                                    .unwrap(),
                                Some((index, p, 0.0))
                            );
                        }
                        let cache = if worker % 2 == 0 {
                            &cloud.data.xz
                        } else {
                            &cloud.data.yz
                        };
                        cache.get().unwrap().nodes.as_ptr() as usize
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(allocations[0], allocations[2]);
        assert_eq!(allocations[1], allocations[3]);
        assert_eq!(
            cloud.data.xz.get().unwrap().nodes.len(),
            cloud.data.points.len()
        );
        assert_eq!(
            cloud.data.yz.get().unwrap().nodes.len(),
            cloud.data.points.len()
        );
    }

    #[test]
    #[ignore = "manual release-mode indexed-versus-scan timing"]
    fn projected_index_query_benchmark() {
        use std::{hint::black_box, time::Instant};
        let cloud = PointCloud3::try_new(
            (0..100_000)
                .map(|i| {
                    point(
                        Real::from(i % 1000),
                        Real::from(i / 1000),
                        Real::from(i * 17 % 997),
                    )
                })
                .collect(),
        )
        .unwrap();
        let origin = point(0.0, 0.0, 0.0);
        for (projection, axes, width) in [
            (PointCloudProjection::Xz, [0, 2], 1000),
            (PointCloudProjection::Yz, [1, 2], 100),
        ] {
            let build_start = Instant::now();
            cloud
                .nearest_projected_relative(projection, origin, [0.0; 2], 5.0)
                .unwrap();
            let build_time = build_start.elapsed();
            let queries: Vec<_> = (0..128)
                .map(|i| {
                    [
                        Real::from(i * 7 % width) + 0.25,
                        Real::from(i * 13 % 997) + 0.25,
                    ]
                })
                .collect();
            let indexed_start = Instant::now();
            let indexed: Vec<_> = queries
                .iter()
                .map(|&q| {
                    black_box(&cloud)
                        .nearest_projected_relative(projection, origin, black_box(q), 5.0)
                        .unwrap()
                })
                .collect();
            let indexed_time = indexed_start.elapsed();
            let scan_start = Instant::now();
            let scanned: Vec<_> = queries
                .iter()
                .map(|&q| {
                    let mut best: Option<(usize, Point3, Real)> = None;
                    for (index, &p) in black_box(cloud.points()).iter().enumerate() {
                        let a = p.to_array();
                        let distance = (a[axes[0]] - q[0]).hypot(a[axes[1]] - q[1]);
                        if distance <= 5.0 && best.is_none_or(|(_, _, d)| distance < d) {
                            best = Some((index, p, distance));
                        }
                    }
                    best
                })
                .collect();
            let scan_time = scan_start.elapsed();
            assert_eq!(indexed, scanned);
            eprintln!(
                "{projection:?}: points=100000 queries=128 build={build_time:?} indexed={indexed_time:?} scan={scan_time:?}"
            );
        }
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
    fn relative_xy_search_matches_local_grid_without_absolute_query_rounding() {
        let local: Vec<_> = (-2..=2)
            .flat_map(|x| (-2..=2).map(move |y| (x, y)))
            .collect();
        for translation in [0.0, 2.0_f64.powi(52), -2.0_f64.powi(52)] {
            let origin = point(translation, translation, 0.0);
            let cloud = PointCloud3::try_new(
                local
                    .iter()
                    .map(|&(x, y)| {
                        point(
                            translation + Real::from(x),
                            translation + Real::from(y),
                            7.0,
                        )
                    })
                    .collect(),
            )
            .unwrap();
            for x in -10..=10 {
                for y in -10..=10 {
                    let offset = [Real::from(x) / 4.0, Real::from(y) / 4.0];
                    for radius in [0.0, 0.25, 0.5, 2.0] {
                        let mut expected: Option<(usize, Real)> = None;
                        for (index, &(px, py)) in local.iter().enumerate() {
                            let distance =
                                (Real::from(px) - offset[0]).hypot(Real::from(py) - offset[1]);
                            if distance <= radius
                                && expected.is_none_or(|(_, best)| distance < best)
                            {
                                expected = Some((index, distance));
                            }
                        }
                        let actual = cloud.nearest_xy_relative(origin, offset, radius).unwrap();
                        assert_eq!(
                            actual.map(|(index, _, distance)| (index, distance)),
                            expected
                        );
                    }
                }
            }
            for invalid in [Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
                for offset in [[invalid, 0.0], [0.0, invalid]] {
                    assert!(cloud.nearest_xy_relative(origin, offset, 1.0).is_err());
                }
            }
        }
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
        for projection in [PointCloudProjection::Xz, PointCloudProjection::Yz] {
            cloud
                .nearest_projected_relative(projection, point(2.0, 3.0, 4.0), [0.0; 2], 0.0)
                .unwrap();
        }
        let transformed = cloud
            .transformed(AffineTransform3::from_translation(
                Vector3::try_new(10.0, -2.0, 1.0).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            transformed.points(),
            [point(10.0, -2.0, 1.0), point(12.0, 1.0, 5.0)]
        );
        assert!(transformed.data.xz.get().is_none() && transformed.data.yz.get().is_none());
        for projection in [
            PointCloudProjection::Xy,
            PointCloudProjection::Xz,
            PointCloudProjection::Yz,
        ] {
            assert_eq!(
                transformed
                    .nearest_projected_relative(projection, point(12.0, 1.0, 5.0), [0.0; 2], 0.0)
                    .unwrap(),
                Some((1, point(12.0, 1.0, 5.0), 0.0))
            );
            assert_eq!(
                cloud
                    .nearest_projected_relative(projection, point(2.0, 3.0, 4.0), [0.0; 2], 0.0)
                    .unwrap(),
                Some((1, point(2.0, 3.0, 4.0), 0.0))
            );
        }
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
