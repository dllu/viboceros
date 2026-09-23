use super::*;
use crate::Vector3;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn optional_point_colors_validate_and_survive_transforms() {
    let points = vec![point(1.0, 2.0, 3.0), point(4.0, 5.0, 6.0)];
    let colors = vec![[10, 20, 30, 0], [40, 50, 60, 128]];
    assert!(matches!(
        PointCloud3::try_with_colors(points.clone(), Some(vec![colors[0]])),
        Err(GeometryError::InvalidPointCloudColorCount)
    ));
    let plain = PointCloud3::try_new(points.clone()).unwrap();
    let cloud = PointCloud3::try_with_colors(points, Some(colors.clone())).unwrap();
    assert_ne!(cloud, plain);
    assert_eq!(cloud.colors(), Some(colors.as_slice()));
    let moved = cloud
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(3.0, 0.0, 0.0).unwrap(),
        ))
        .unwrap();
    assert_eq!(moved.colors(), Some(colors.as_slice()));
    assert_eq!(moved.points(), [point(4.0, 2.0, 3.0), point(7.0, 5.0, 6.0)]);
}

#[test]
fn optional_normals_values_and_order_validate_and_survive_transforms() {
    let points = vec![point(1.0, 2.0, 3.0), point(4.0, 5.0, 6.0)];
    let normals = vec![
        Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
        Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
    ];
    let channels = PointCloudChannels {
        normals: Some(normals.clone()),
        values: Some(vec![0.5, -20.0]),
        ordered: true,
        ..PointCloudChannels::default()
    };
    assert_eq!(
        PointCloud3::try_with_channels(
            points.clone(),
            PointCloudChannels {
                normals: Some(vec![normals[0]]),
                ..channels.clone()
            },
        ),
        Err(GeometryError::InvalidPointCloudNormalCount)
    );
    assert_eq!(
        PointCloud3::try_with_channels(
            points.clone(),
            PointCloudChannels {
                values: Some(vec![0.5]),
                ..channels.clone()
            },
        ),
        Err(GeometryError::InvalidPointCloudValueCount)
    );
    assert!(matches!(
        PointCloud3::try_with_channels(
            points.clone(),
            PointCloudChannels {
                values: Some(vec![f64::NAN, 0.0]),
                ..channels.clone()
            },
        ),
        Err(GeometryError::NonFinite { .. })
    ));
    let cloud = PointCloud3::try_with_channels(points, channels.clone()).unwrap();
    let moved = cloud
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(3.0, 0.0, 0.0).unwrap(),
        ))
        .unwrap();
    assert_eq!(moved.channels(), &channels);
    assert_eq!(moved.normals(), Some(normals.as_slice()));
    assert_eq!(moved.values(), Some(&[0.5, -20.0][..]));
    assert!(moved.is_ordered());
    assert_ne!(
        cloud,
        PointCloud3::try_new(cloud.points().to_vec()).unwrap()
    );
}

#[test]
fn square_indexes_match_exhaustive_queries_in_each_plane() {
    for translation in [0., 2.0_f64.powi(52), -2.0_f64.powi(52)] {
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
                let offset = [Real::from(i % 13 - 6) / 2., Real::from(i * 5 % 17 - 8) / 2.];
                for radius in [0., 0.5, 2., 100.] {
                    let mut expected: Option<(usize, Point3, Real)> = None;
                    for (index, &p) in points.iter().enumerate() {
                        let coordinates = p.to_array();
                        let x = (coordinates[axes[0]] - translation) - offset[0];
                        let y = (coordinates[axes[1]] - translation) - offset[1];
                        let distance = x.hypot(y);
                        if x.abs() <= radius
                            && y.abs() <= radius
                            && expected.is_none_or(|(_, _, best)| distance < best)
                        {
                            expected = Some((index, p, distance));
                        }
                    }
                    assert_eq!(
                        cloud
                            .nearest_projected_in_box_relative(projection, origin, offset, radius)
                            .unwrap(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn square_admission_does_not_confuse_euclidean_ranking_or_circular_queries() {
    let corner = point(1., 1., 7.);
    let cloud =
        PointCloud3::try_new(vec![point(1.01, 0., 0.), corner, point(-1., -1., 0.)]).unwrap();
    let origin = point(0., 0., 0.);
    assert_eq!(cloud.nearest_xy(origin, 1.).unwrap(), None);
    assert_eq!(
        cloud
            .nearest_projected_in_box_relative(PointCloudProjection::Xy, origin, [0.; 2], 1.)
            .unwrap(),
        Some((1, corner, 2.0_f64.sqrt()))
    );
    assert_eq!(
        cloud
            .nearest_projected_in_box_relative(
                PointCloudProjection::Xy,
                origin,
                [0.; 2],
                1.0_f64.next_down()
            )
            .unwrap(),
        None
    );
}

#[test]
fn square_queries_validate_before_initialization_and_reuse_circular_indexes() {
    let cloud = PointCloud3::try_new(vec![point(0., 1., 2.), point(3., 4., 5.)]).unwrap();
    let origin = point(0., 0., 0.);
    for radius in [-1., Real::NAN, Real::INFINITY] {
        assert!(
            cloud
                .nearest_projected_in_box_relative(
                    PointCloudProjection::Xz,
                    origin,
                    [0.; 2],
                    radius
                )
                .is_err()
        );
    }
    for offset in [[Real::NAN, 0.], [0., Real::INFINITY]] {
        assert!(
            cloud
                .nearest_projected_in_box_relative(PointCloudProjection::Xz, origin, offset, 1.)
                .is_err()
        );
    }
    assert!(cloud.data.xz.get().is_none());
    cloud
        .nearest_projected_relative(PointCloudProjection::Xz, origin, [0.; 2], 10.)
        .unwrap();
    let allocation = cloud.data.xz.get().unwrap().nodes.as_ptr();
    for offset in [[0.; 2], [1.; 2], [7.; 2]] {
        cloud
            .nearest_projected_in_box_relative(PointCloudProjection::Xz, origin, offset, 10.)
            .unwrap();
        assert_eq!(cloud.data.xz.get().unwrap().nodes.as_ptr(), allocation);
    }
    assert!(cloud.data.yz.get().is_none());
}

#[test]
fn square_distance_overflow_is_not_a_false_miss_or_a_masked_finite_target() {
    let huge = point(Real::MAX, Real::MAX, 0.);
    let origin = point(0., 0., 0.);
    let cloud = PointCloud3::try_new(vec![huge]).unwrap();
    assert!(matches!(
        cloud.nearest_projected_in_box_relative(
            PointCloudProjection::Xy,
            origin,
            [0.; 2],
            Real::MAX
        ),
        Err(GeometryError::NonFinite {
            context: "point cloud projected distance"
        })
    ));
    let finite = point(0., Real::MAX, 0.);
    let cloud =
        PointCloud3::try_new(vec![huge, finite, point(-Real::MAX, -Real::MAX, 0.)]).unwrap();
    assert_eq!(
        cloud
            .nearest_projected_in_box_relative(PointCloudProjection::Xy, origin, [0.; 2], Real::MAX)
            .unwrap(),
        Some((1, finite, Real::MAX))
    );
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
                        if distance <= radius && expected.is_none_or(|(_, _, best)| distance < best)
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
                        if distance <= radius && expected.is_none_or(|(_, best)| distance < best) {
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
fn coincident_projections_preserve_earliest_source_order() {
    for projection in [
        PointCloudProjection::Xy,
        PointCloudProjection::Xz,
        PointCloudProjection::Yz,
    ] {
        let axes = projection.axes();
        let points = (0..1024)
            .map(|i| {
                let mut coordinates = [0.0; 3];
                coordinates[usize::from(axes[0])] = (i % 7) as Real;
                coordinates[usize::from(axes[2])] = ((i * 313) % 1024) as Real;
                Point3::try_new(coordinates[0], coordinates[1], coordinates[2]).unwrap()
            })
            .collect::<Vec<_>>();
        let cloud = PointCloud3::try_new(points.clone()).unwrap();
        for (group, &candidate) in points.iter().take(7).enumerate() {
            for offset in [[group as Real, 0.0], [group as Real, 0.25]] {
                for radius in [0.0, 0.25, 10.0] {
                    let expected = (offset[1] <= radius).then_some((group, candidate, offset[1]));
                    assert_eq!(
                        cloud
                            .nearest_projected_relative(
                                projection,
                                point(0.0, 0.0, 0.0),
                                offset,
                                radius
                            )
                            .unwrap(),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "release-mode timing diagnostic"]
fn coincident_projected_index_query_benchmark() {
    use std::{hint::black_box, time::Instant};
    for projection in [
        PointCloudProjection::Xy,
        PointCloudProjection::Xz,
        PointCloudProjection::Yz,
    ] {
        let axes = projection.axes();
        let points = (0..100_000)
            .map(|i| {
                let mut coordinates = [0.0; 3];
                coordinates[usize::from(axes[2])] = ((i * 7919) % 100_000) as Real;
                Point3::try_new(coordinates[0], coordinates[1], coordinates[2]).unwrap()
            })
            .collect::<Vec<_>>();
        let cloud = PointCloud3::try_new(points).unwrap();
        let origin = point(0.0, 0.0, 0.0);
        cloud
            .nearest_projected_relative(projection, origin, [0.0; 2], 0.0)
            .unwrap();
        let start = Instant::now();
        for _ in 0..128 {
            assert_eq!(
                black_box(&cloud)
                    .nearest_projected_relative(projection, origin, black_box([0.0; 2]), 0.0)
                    .unwrap(),
                Some((0, cloud.points()[0], 0.0))
            );
        }
        eprintln!(
            "{projection:?}: coincident points=100000 queries=128 indexed={:?}",
            start.elapsed()
        );
    }
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

#[test]
fn arbitrary_frame_queries_match_brute_force_with_square_and_circle_capture() {
    fn random_unit(state: &mut u64) -> Real {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (*state >> 11) as Real / ((1_u64 << 53) as Real)
    }
    let mut state = 0x1234_5678_9abc_def0;
    for translation in [0.0, 2.0_f64.powi(52)] {
        let frame = Frame3::try_from_directions(
            point(translation, -translation, translation),
            Vector3::try_new(1.0, 1.0, 0.0).unwrap(),
            Vector3::try_new(-1.0, 1.0, 1.0).unwrap(),
            crate::Tolerance::DEFAULT,
        )
        .unwrap();
        let points = (0..401)
            .map(|index| {
                let coordinates = if index % 53 == 0 {
                    [2.0, -3.0, index as Real]
                } else {
                    [
                        random_unit(&mut state) * 80.0 - 40.0,
                        random_unit(&mut state) * 80.0 - 40.0,
                        random_unit(&mut state) * 80.0 - 40.0,
                    ]
                };
                frame.point_at(coordinates).unwrap()
            })
            .collect::<Vec<_>>();
        let cloud = PointCloud3::try_new(points.clone()).unwrap();
        for _ in 0..128 {
            let offset = [
                random_unit(&mut state) * 100.0 - 50.0,
                random_unit(&mut state) * 100.0 - 50.0,
            ];
            let radius = random_unit(&mut state) * 30.0;
            for square in [false, true] {
                let expected = points
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &candidate)| {
                        let projected = frame.projected_coordinates_of(candidate).ok()?;
                        let delta = [projected[0] - offset[0], projected[1] - offset[1]];
                        let distance = delta[0].hypot(delta[1]);
                        let captured = if square {
                            delta[0].abs().max(delta[1].abs()) <= radius
                        } else {
                            distance <= radius
                        };
                        captured.then_some((distance, index, candidate))
                    })
                    .min_by(|left, right| {
                        left.0
                            .total_cmp(&right.0)
                            .then_with(|| left.1.cmp(&right.1))
                    })
                    .map(|(distance, index, candidate)| (index, candidate, distance));
                let actual = if square {
                    cloud.nearest_projected_in_frame_box_relative(frame, offset, radius)
                } else {
                    cloud.nearest_projected_frame_relative(frame, offset, radius)
                };
                assert_eq!(
                    actual.unwrap(),
                    expected,
                    "translation={translation} offset={offset:?} radius={radius} square={square}"
                );
            }
        }
        assert!(cloud.data.spatial_bounds.get().is_some());
    }
}

#[test]
fn frame_index_does_not_require_representable_normal_depth() {
    let frame = Frame3::try_from_directions(
        point(0.0, 0.0, -Real::MAX),
        Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
        Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
        crate::Tolerance::DEFAULT,
    )
    .unwrap();
    let near = point(1.0, 2.0, Real::MAX);
    let far = point(4.0, 5.0, -Real::MAX);
    let cloud = PointCloud3::try_new(vec![far, near]).unwrap();
    assert_eq!(
        cloud
            .nearest_projected_frame_relative(frame, [1.0, 2.0], 0.0)
            .unwrap(),
        Some((1, near, 0.0))
    );
}

#[test]
fn frame_index_preserves_earliest_source_on_exact_projection_ties() {
    let frame = Frame3::try_from_directions(
        point(10.0, 20.0, 30.0),
        Vector3::try_new(1.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(-1.0, 1.0, 1.0).unwrap(),
        crate::Tolerance::DEFAULT,
    )
    .unwrap();
    let tied = frame.point_at([2.0, -3.0, 4.0]).unwrap();
    let cloud =
        PointCloud3::try_new(vec![frame.point_at([50.0, 50.0, 0.0]).unwrap(), tied, tied]).unwrap();
    let offset = frame.projected_coordinates_of(tied).unwrap();
    for result in [
        cloud.nearest_projected_frame_relative(frame, offset, 0.0),
        cloud.nearest_projected_in_frame_box_relative(frame, offset, 0.0),
    ] {
        assert_eq!(result.unwrap(), Some((1, tied, 0.0)));
    }
    assert_eq!(
        cloud.nearest_projected_frame_relative(frame, offset, -1.0),
        Err(GeometryError::InvalidPointCloudSearchRadius)
    );
}

#[test]
#[ignore = "release-mode timing diagnostic"]
fn oblique_frame_query_benchmark() {
    use std::{hint::black_box, time::Instant};
    let frame = Frame3::try_from_directions(
        point(0.0, 0.0, 0.0),
        Vector3::try_new(1.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(-1.0, 1.0, 1.0).unwrap(),
        crate::Tolerance::DEFAULT,
    )
    .unwrap();
    let points = (0..100_000)
        .map(|index| {
            let x = (index % 316) as Real - 158.0;
            let y = (index / 316) as Real - 158.0;
            point(x, y, (x * 0.1).sin() + (y * 0.1).cos())
        })
        .collect::<Vec<_>>();
    let cloud = PointCloud3::try_new(points).unwrap();
    let offsets = (0..128)
        .map(|index| [index as Real * 1.3 - 80.0, index as Real * 0.7 - 45.0])
        .collect::<Vec<_>>();
    cloud
        .nearest_projected_frame_relative(frame, offsets[0], 1.0)
        .unwrap();
    let start = Instant::now();
    let indexed = offsets
        .iter()
        .map(|offset| {
            black_box(&cloud)
                .nearest_projected_frame_relative(frame, black_box(*offset), 1.0)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let indexed_time = start.elapsed();
    let start = Instant::now();
    let scanned = offsets
        .iter()
        .map(|offset| {
            cloud
                .points()
                .iter()
                .enumerate()
                .filter_map(|(index, &point)| {
                    let projected = frame.projected_coordinates_of(point).ok()?;
                    let distance = (projected[0] - offset[0]).hypot(projected[1] - offset[1]);
                    (distance <= 1.0).then_some((distance, index, point))
                })
                .min_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                })
                .map(|(distance, index, point)| (index, point, distance))
        })
        .collect::<Vec<_>>();
    assert_eq!(indexed, scanned);
    eprintln!(
        "oblique cloud points=100000 queries=128 indexed={indexed_time:?} scan={:?}",
        start.elapsed()
    );
}
