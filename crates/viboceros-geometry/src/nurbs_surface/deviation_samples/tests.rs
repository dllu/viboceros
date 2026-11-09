use super::*;
fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane(z: Real) -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![
            point(0., 0., z),
            point(1., 0., z),
            point(0., 1., z),
            point(1., 1., z),
        ],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-12, 1e-12, 1e-9).unwrap()
}

#[test]
fn translated_images_keep_original_charts_and_valid_witness_points() {
    let a = plane(0.).try_reparameterized(2. ..=5., -7. ..=-1.).unwrap();
    let b = plane(0.125)
        .try_reparameterized(0.01..=0.02, 3e6..=7e6)
        .unwrap();
    let originals = (a.clone(), b.clone());
    let samples = a.deviation_samples(&b, [4, 3], tolerance()).unwrap();
    for summary in [
        samples.same_parameters,
        samples.first_to_second,
        samples.second_to_first,
    ] {
        assert_eq!(summary.sample_count, 20);
        assert!((summary.maximum.distance - 0.125).abs() < 1e-12);
    }
    let w = samples.first_to_second.maximum;
    assert!(
        a.evaluate(w.source_parameters[0], w.source_parameters[1])
            .unwrap()
            .distance_to(w.source_point)
            .unwrap()
            < 1e-12
    );
    assert!(
        b.evaluate(w.target_parameters[0], w.target_parameters[1])
            .unwrap()
            .distance_to(w.target_point)
            .unwrap()
            < 1e-12
    );
    assert_eq!((a, b), originals);
}

#[test]
fn a_grid_can_miss_a_complete_interior_spike() {
    let xs = [0., 1. / 64., 2. / 64., 3. / 64., 1.];
    let a = NurbsSurface::try_new(
        1,
        1,
        5,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                xs.into_iter()
                    .enumerate()
                    .map(move |(i, x)| point(x, y, if i == 2 { 1. } else { 0. }))
            })
            .collect(),
        vec![0., 0., 1. / 64., 2. / 64., 3. / 64., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let b = plane(0.);
    let samples = a.deviation_samples(&b, [16, 2], tolerance()).unwrap();
    assert!(samples.same_parameters.maximum.distance < 1e-12);
    assert!(samples.first_to_second.maximum.distance < 1e-12);
    assert!(samples.second_to_first.maximum.distance < 1e-12);
    assert!(
        a.parameter_surface_deviation_bound(&b, 0.1)
            .unwrap()
            .is_none()
    );
    let refined = a.deviation_samples(&b, [64, 2], tolerance()).unwrap();
    assert_eq!(refined.first_to_second.maximum.distance, 1.);
}

#[test]
fn failed_local_refinements_retain_finite_point_pairs() {
    let a = plane(0.);
    let b = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![point(3., 4., 5.); 4],
        vec![0., 0., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let result = a.deviation_samples(&b, [2, 2], tolerance()).unwrap();
    assert_eq!(result.first_to_second.refinement_failures, 9);
    assert_eq!(
        result.first_to_second.maximum.target_point,
        point(3., 4., 5.)
    );
    assert!(result.first_to_second.maximum.distance.is_finite());
    assert!(result.first_to_second.maximum.distance <= result.same_parameters.maximum.distance);
}

#[test]
fn divisions_and_surface_complexity_bound_work() {
    let a = plane(0.);
    for n in [[0, 1], [1, 0], [65, 1], [usize::MAX, 1]] {
        assert!(a.deviation_samples(&a, n, tolerance()).is_err());
    }
    assert_eq!(
        a.deviation_samples(&a, [64, 64], tolerance())
            .unwrap()
            .same_parameters
            .sample_count,
        4225
    );
    let large = NurbsSurface::try_new(
        17,
        1,
        18,
        2,
        vec![point(0., 0., 0.); 36],
        [vec![0.; 18], vec![1.; 18]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(matches!(
        large.deviation_samples(&a, [1, 1], tolerance()),
        Err(GeometryError::SurfaceApproximationLimit)
    ));
    let axis = |n: usize| {
        [
            vec![0.],
            (0..n).map(|i| i as Real / (n - 1) as Real).collect(),
            vec![1.],
        ]
        .concat()
    };
    let large = NurbsSurface::try_new(
        1,
        1,
        65,
        64,
        (0..64)
            .flat_map(|j| (0..65).map(move |i| point(i as Real / 64., j as Real / 63., 0.)))
            .collect(),
        axis(65),
        axis(64),
    )
    .unwrap();
    assert!(matches!(
        large.deviation_samples(&a, [1, 1], tolerance()),
        Err(GeometryError::SurfaceApproximationLimit)
    ));
}

#[test]
fn seeded_continuation_clamps_finite_seeds_and_rejects_nonfinite_ones() {
    let a = plane(0.);
    let p = point(0.25, 0.75, 1.);
    let (u, v) = a
        .closest_parameters_from_seed(p, [-10., 10.], tolerance())
        .unwrap();
    assert!((u - 0.25).abs() < 1e-12 && (v - 0.75).abs() < 1e-12);
    for bad in [Real::NAN, Real::INFINITY, Real::NEG_INFINITY] {
        assert!(
            a.closest_parameters_from_seed(p, [bad, 0.], tolerance())
                .is_err()
        );
    }
}
