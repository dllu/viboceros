use super::*;

fn endpoint(index: usize, coordinates: [Real; 3]) -> Endpoint {
    Endpoint {
        curve: index / 2,
        start: index.is_multiple_of(2),
        point: Point3::try_new(coordinates[0], coordinates[1], coordinates[2]).unwrap(),
        outward_tangent: None,
    }
}

fn options(tolerance: Real, preserve_direction: bool) -> CurveJoinOptions {
    CurveJoinOptions {
        tolerance,
        preserve_direction,
        style: CurveJoinStyle::Batch,
    }
}

fn exhaustive(endpoints: &[Endpoint], options: CurveJoinOptions) -> Vec<(usize, usize, Real)> {
    let mut pairs = Vec::new();
    for (right, b) in endpoints.iter().enumerate() {
        for (left, a) in endpoints[..right].iter().enumerate() {
            if a.curve == b.curve || (options.preserve_direction && a.start == b.start) {
                continue;
            }
            let distance = (a.point.x() - b.point.x())
                .hypot(a.point.y() - b.point.y())
                .hypot(a.point.z() - b.point.z());
            if distance <= options.tolerance {
                pairs.push((left, right, distance));
            }
        }
    }
    pairs.sort_by_key(|&(a, b, _)| (a, b));
    pairs
}

fn check(endpoints: &[Endpoint], tolerance: Real) {
    for preserve_direction in [false, true] {
        let options = options(tolerance, preserve_direction);
        let mut found = find_candidates(endpoints, options)
            .unwrap()
            .iter()
            .map(|c| (c.left, c.right, c.distance))
            .collect::<Vec<_>>();
        found.sort_by_key(|&(a, b, _)| (a, b));
        assert_eq!(found, exhaustive(endpoints, options), "radius {tolerance}");
    }
}

#[test]
fn endpoint_search_matches_exhaustive_at_binary64_boundaries() {
    let tiny = Real::from_bits(1);
    for axis in 0..3 {
        for scale in [tiny, Real::MIN_POSITIVE, 1e-100, 0.01, 1.0, 1e100, 1e300] {
            for sign in [-1.0, 1.0] {
                let mut coordinates = vec![-9_007_199_254_740_992.0 * scale, 0.0, -0.0];
                for k in -12..=12 {
                    let t = k as Real * scale;
                    coordinates.extend([t.next_down(), t, t.next_up()]);
                }
                let points = coordinates
                    .into_iter()
                    .filter(|x| x.is_finite())
                    .enumerate()
                    .map(|(index, x)| {
                        let mut p = [0.0; 3];
                        p[axis] = sign * x;
                        endpoint(index, p)
                    })
                    .collect::<Vec<_>>();
                for radius in [0.0, scale.next_down(), scale, scale.next_up()] {
                    check(&points, radius);
                }
            }
        }
    }
}

#[test]
fn endpoint_search_matches_exhaustive_on_seeded_random_multiscale_clouds() {
    let mut state = 0x2026_0920_cad0_0001u64;
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in 0..256 {
        let scale = [1e-300, 1e-100, 0.001, 1.0, 1e100, 1e300][i % 6];
        let mut points: Vec<Endpoint> = Vec::new();
        for j in 0..97 {
            let p = if j % 7 == 0 && j > 0 {
                points[j - 1].point.to_array()
            } else {
                std::array::from_fn(|_| (random() % 33) as Real * scale)
            };
            points.push(endpoint(j, p));
        }
        points.push(endpoint(97, [-Real::MAX, 0.0, Real::MAX]));
        points.push(endpoint(98, [Real::MAX, 0.0, -Real::MAX]));
        for radius in [0.0, scale, 3.0 * scale, Real::MAX] {
            check(&points, radius);
        }
    }
}

fn workload(name: &str, count: usize) -> (Vec<Endpoint>, Real) {
    let (scale, radius) = if name == "tiny_radius_grid" {
        (1.0, 1e-300)
    } else {
        (1.0, 0.01)
    };
    let points = (0..count)
        .map(|i| {
            let pair = i / 2;
            let p = match name {
                "chain" => [(i / 2 + i % 2) as Real, 0.0, 0.0],
                "grid_3d" => [
                    (pair % 32) as Real,
                    ((pair / 32) % 32) as Real,
                    (pair / 1024) as Real,
                ],
                "translated_grid" => [
                    1e16,
                    1e16 + 8.0 * (pair % 200) as Real,
                    1e16 + 8.0 * (pair / 200) as Real,
                ],
                "tiny_radius_grid" => [0.0, (pair % 200) as Real, (pair / 200) as Real],
                _ => unreachable!(),
            };
            // Coincident endpoints belong to distinct input curves.
            let mut value = endpoint(i, p.map(|v| v * scale));
            value.curve = i;
            value
        })
        .collect();
    (points, radius)
}

#[test]
fn endpoint_search_handles_large_planar_and_spatial_sets_within_work_budget() {
    for name in ["chain", "grid_3d", "translated_grid", "tiny_radius_grid"] {
        let (points, tolerance) = workload(name, 100_000);
        let pairs = find_candidates(&points, options(tolerance, false)).unwrap();
        assert_eq!(pairs.len(), if name == "chain" { 49_999 } else { 50_000 });
    }
}

#[test]
fn endpoint_search_limits_dense_candidates_and_incompatible_pair_work() {
    for radius in [0.0, 0.01] {
        let mut points = (0..1500)
            .map(|i| {
                let mut p = endpoint(i, [0.0; 3]);
                p.curve = i;
                p
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            find_candidates(&points, options(radius, false)),
            Err(GeometryError::CurveJoinLimit {
                resource: "endpoint candidates",
                maximum: MAX_JOIN_CANDIDATES,
            })
        ));
        points.resize(6000, endpoint(0, [0.0; 3]));
        for p in &mut points {
            p.curve = 0;
        }
        assert!(matches!(
            find_candidates(&points, options(radius, false)),
            Err(GeometryError::CurveJoinLimit {
                resource: "endpoint comparisons",
                maximum: MAX_JOIN_SCANS,
            })
        ));
    }
    check(&[], 0.0);
    check(&[], 1.0);
    check(&[endpoint(0, [0.0; 3])], 1.0);
}

#[test]
#[ignore = "manual endpoint-search timing; no wall-clock CI threshold"]
fn endpoint_search_benchmark() {
    use std::{hint::black_box, time::Instant};
    for name in ["chain", "grid_3d", "translated_grid", "tiny_radius_grid"] {
        let (points, tolerance) = workload(name, 50_000);
        let mut times = Vec::new();
        for _ in 0..7 {
            let now = Instant::now();
            let result = find_candidates(black_box(&points), options(tolerance, false));
            times.push(now.elapsed().as_secs_f64());
            match result {
                Ok(pairs) => assert_eq!(pairs.len(), if name == "chain" { 24_999 } else { 25_000 }),
                Err(error) => {
                    println!("{name}: {error}");
                    times.clear();
                    break;
                }
            }
        }
        if !times.is_empty() {
            times.sort_by(Real::total_cmp);
            println!("{name}: median {:.6} s; samples {times:?}", times[3]);
        }
    }
}
