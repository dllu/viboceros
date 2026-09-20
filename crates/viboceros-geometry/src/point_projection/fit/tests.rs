use super::*;

fn p(a: [f64; 3]) -> Point3 {
    Point3::try_from(a).unwrap()
}
fn v(a: [f64; 3]) -> Vector3 {
    Vector3::try_from(a).unwrap()
}
fn symmetric_cloud(scale: f64) -> Vec<Point3> {
    let center = [1., 2., 3.];
    [[5., 5., 0.], [-2., 2., 8.], [0.25, -0.25, 0.125]]
        .into_iter()
        .flat_map(|d| {
            [-1., 1.].map(|sign| p(std::array::from_fn(|i| (center[i] + sign * d[i]) * scale)))
        })
        .collect()
}

#[test]
fn orthogonal_scatter_axes_determine_the_least_squares_plane_across_scales() {
    for exponent in [-1000, -500, 0, 500, 1000] {
        let scale = 2_f64.powi(exponent);
        let points = symmetric_cloud(scale);
        let plane = PointProjection3::onto_best_fit_plane(&points).unwrap();
        let expected =
            PointProjection3::onto_plane(p([scale, 2. * scale, 3. * scale]), v([2., -2., 1.]))
                .unwrap();
        for point in points {
            let actual = plane.project(point).unwrap().to_array();
            let expected = expected.project(point).unwrap().to_array();
            for i in 0..3 {
                assert!(
                    ((actual[i] - expected[i]) / scale).abs() < 2e-14,
                    "scale {exponent}: {actual:?} != {expected:?}"
                );
            }
        }
    }
}

#[test]
fn exact_coplanarity_preserves_extreme_thin_clouds_and_degenerate_sets() {
    let huge = 2_f64.powi(1023);
    let tiny = f64::from_bits(1);
    for points in [
        vec![p([1., 2., 3.]); 4],
        vec![p([-huge, -huge, 2.]), p([huge, huge, 2.])],
        vec![
            p([0., 0., 0.]),
            p([huge, tiny, 0.]),
            p([-huge, tiny, 0.]),
            p([0., tiny, 0.]),
        ],
        vec![
            p([0.; 3]),
            p([1., 2., 3.]),
            p([2., 4., 6.]),
            p([7., 14., 21.]),
        ],
    ] {
        let plane = PointProjection3::onto_best_fit_plane(&points).unwrap();
        for point in points {
            assert_eq!(plane.project(point).unwrap(), point);
        }
    }
}

#[test]
fn duplicate_occurrences_are_weighted_and_centroid_is_not_rounded_before_projection() {
    let points = [
        [4., 0., 0.],
        [-4., 0., 0.],
        [0., 4., 0.],
        [0., -4., 0.],
        [0., 0., 1.],
        [0., 0., 1.],
    ]
    .map(p);
    let plane = PointProjection3::onto_best_fit_plane(&points).unwrap();
    for point in points {
        let q = plane.project(point).unwrap();
        assert_eq!(q.z(), 1. / 3.);
        assert_eq!(q.x(), point.x());
        assert_eq!(q.y(), point.y());
    }
    let third = Rational::new(1.into(), 3.into());
    let plane = PointProjection3::new_exact(
        [third.clone(), third, Rational::zero()],
        [rational(1.), rational(2.), rational(0.)],
        false,
    )
    .unwrap();
    assert_eq!(
        plane.project(p([0.; 3])).unwrap(),
        p([1. / 5., 2. / 5., 0.])
    );
}

#[test]
fn fit_is_permutation_invariant_for_a_separated_normal() {
    let mut points = symmetric_cloud(1.);
    let baseline = PointProjection3::onto_best_fit_plane(&points).unwrap();
    let query = p([7., 8., 9.]);
    for _ in 0..points.len() {
        points.rotate_left(1);
        for points in [points.clone(), points.iter().rev().copied().collect()] {
            let actual = PointProjection3::onto_best_fit_plane(&points)
                .unwrap()
                .project(query)
                .unwrap();
            assert!(
                actual
                    .distance_to(baseline.project(query).unwrap())
                    .unwrap()
                    < 1e-13
            );
        }
    }
}

#[test]
fn isotropic_scatter_accepts_any_plane_through_the_centroid_with_optimal_residual() {
    let points = [[1., 1., 1.], [1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.]].map(p);
    let plane = PointProjection3::onto_best_fit_plane(&points).unwrap();
    assert_eq!(plane.project(p([0.; 3])).unwrap(), p([0.; 3]));
    let error: f64 = points
        .into_iter()
        .map(|p| p.distance_to(plane.project(p).unwrap()).unwrap().powi(2))
        .sum();
    assert!((error - 4.).abs() < 1e-13);
}

#[test]
fn fit_rejects_empty_oversized_and_unresolved_full_range_inputs() {
    assert!(matches!(
        PointProjection3::onto_best_fit_plane(&[]),
        Err(GeometryError::EmptyPointSet)
    ));
    assert!(matches!(
        PointProjection3::onto_best_fit_plane(&vec![p([0.; 3]); MAX_PLANE_FIT_POINTS + 1]),
        Err(GeometryError::PlaneFitResourceLimit { .. })
    ));
    let (h, t) = (2_f64.powi(1023), f64::from_bits(1));
    assert!(matches!(
        PointProjection3::onto_best_fit_plane(&[
            p([0.; 3]),
            p([h, 0., 0.]),
            p([0., t, 0.]),
            p([0., 0., t])
        ]),
        Err(GeometryError::PlaneFitNumericalRange)
    ));
}
