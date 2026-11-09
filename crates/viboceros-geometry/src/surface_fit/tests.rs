use super::*;
use crate::{SurfaceKnotDirection, WeightedPoint3};

fn source(rational: bool, origin: Real) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        2,
        3,
        3,
        4,
        (0..4)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            origin + i as Real,
                            j as Real,
                            (i * i + j * j + i * j) as Real / 8.,
                        )
                        .unwrap(),
                        if rational {
                            1. + ((i + 2 * j) % 3) as Real / 2.
                        } else {
                            1.
                        },
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![-4., -4., -4., 2., 2., 2.],
        vec![3., 3., 3., 3., 11., 11., 11., 11.],
    )
    .unwrap()
    .try_insert_knot_u(-1., 2)
    .unwrap()
    .try_insert_knot_v(7., 3)
    .unwrap()
}

fn knots(degree: usize, a: Real, b: Real) -> Vec<Real> {
    [vec![a; degree + 1], vec![b; degree + 1]].concat()
}

#[test]
fn streamed_interpolation_keeps_scalar_control_bits_including_exact_recovery() {
    for origin in [0., 1e9] {
        for rational in [false, true] {
            let original = source(rational, origin);
            for gauge in [1., -2., 1e-300] {
                let shape = NurbsSurface::try_new_rational(
                    original.degree_u(),
                    original.degree_v(),
                    original.control_point_count_u(),
                    original.control_point_count_v(),
                    original
                        .control_points()
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let weight = if gauge == 1e-300 && i % 2 == 1 {
                                1e300
                            } else {
                                gauge * p.weight()
                            };
                            WeightedPoint3::try_new(p.point(), weight).unwrap()
                        })
                        .collect(),
                    original.knots_u().to_vec(),
                    original.knots_v().to_vec(),
                )
                .unwrap();
                for degree in [[2, 3], [3, 3], [5, 4]] {
                    let u = knots(degree[0], -4., 2.);
                    let v = knots(degree[1], 3., 11.);
                    let actual =
                        try_interpolate_surface_in_basis(&shape, degree, [&u, &v]).unwrap();
                    let scalar = interpolate_with_sampler(
                        &shape,
                        degree,
                        [&u, &v],
                        &mut crate::surface_rebuild::portable_solve,
                        &mut |u, v| {
                            let mut samples = Vec::new();
                            for &y in v {
                                for &x in u {
                                    samples.push(shape.evaluate(x, y)?.to_array());
                                }
                            }
                            Ok(samples)
                        },
                    )
                    .unwrap_or_else(|_| panic!("scalar interpolation failed"));
                    let bits = |s: &NurbsSurface| {
                        s.control_points()
                            .iter()
                            .map(|p| {
                                (
                                    p.point().to_array().map(Real::to_bits),
                                    p.weight().to_bits(),
                                )
                            })
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(
                        bits(&actual),
                        bits(&scalar),
                        "{origin}, {rational}, {gauge}, {degree:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn degree_changes_reproduce_complete_polynomial_tensors_without_changing_the_source() {
    for origin in [0., 1e9] {
        let original = source(false, origin);
        let before = original.clone();
        for degree in [[2, 3], [3, 3], [5, 4]] {
            let u = knots(degree[0], -4., 2.);
            let v = knots(degree[1], 3., 11.);
            let limit = if origin == 0. { 1e-11 } else { 1e-5 };
            let result = try_fit_surface_in_basis(&original, degree, [&u, &v], limit)
                .unwrap()
                .unwrap();
            assert_eq!(
                (result.surface.degree_u(), result.surface.degree_v()),
                (degree[0], degree[1])
            );
            assert_eq!(
                (
                    result.surface.control_point_count_u(),
                    result.surface.control_point_count_v()
                ),
                (degree[0] + 1, degree[1] + 1)
            );
            assert!(
                result
                    .surface
                    .control_points()
                    .iter()
                    .all(|p| p.weight() == 1.)
            );
            assert!(result.maximum_deviation_bound <= limit);
            assert_eq!(result.surface.domain_u(), original.domain_u());
            assert_eq!(result.surface.domain_v(), original.domain_v());
        }
        assert_eq!(original, before);
    }
}

#[test]
fn rational_inputs_fit_cartesian_positions_and_are_admitted_only_with_a_complete_bound() {
    let original = source(true, 0.);
    let u = knots(3, -4., 2.);
    let v = knots(3, 3., 11.);
    let candidate = try_interpolate_surface_in_basis(&original, [3, 3], [&u, &v]).unwrap();
    let loose = try_fit_surface_in_basis(&original, [3, 3], [&u, &v], 1.)
        .unwrap()
        .unwrap();
    assert_eq!(candidate, loose.surface);
    assert!(loose.maximum_deviation_bound < 1.);
    assert!(
        try_fit_surface_in_basis(&original, [3, 3], [&u, &v], 1e-9)
            .unwrap()
            .is_none()
    );
    let negative = NurbsSurface::try_new_rational(
        original.degree_u(),
        original.degree_v(),
        original.control_point_count_u(),
        original.control_point_count_v(),
        original
            .control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), -2. * p.weight()).unwrap())
            .collect(),
        original.knots_u().to_vec(),
        original.knots_v().to_vec(),
    )
    .unwrap();
    assert!(
        candidate
            .parameter_surface_deviation_bound(
                &try_interpolate_surface_in_basis(&negative, [3, 3], [&u, &v]).unwrap(),
                1e-10
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_peak_invisible_at_every_interpolation_station_is_not_accepted() {
    let source = NurbsSurface::try_new(
        6,
        1,
        7,
        2,
        (0..2)
            .flat_map(|j| {
                (0..7).map(move |i| {
                    Point3::try_new(i as Real / 6., j as Real, if i == 3 { 1. } else { 0. })
                        .unwrap()
                })
            })
            .collect(),
        knots(6, 0., 1.),
        knots(1, 0., 1.),
    )
    .unwrap();
    let u = knots(1, 0., 1.);
    let v = u.clone();
    let flat = try_interpolate_surface_in_basis(&source, [1, 1], [&u, &v]).unwrap();
    assert!(flat.control_points().iter().all(|p| p.point().z() == 0.));
    assert!(source.evaluate(0.5, 0.5).unwrap().z() > 0.3);
    assert!(
        try_fit_surface_in_basis(&source, [1, 1], [&u, &v], 0.01)
            .unwrap()
            .is_none()
    );
}

#[test]
fn exact_zero_restores_constant_channels_and_corners() {
    let original = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        vec![Point3::try_new(2., 3., 4.).unwrap(); 9],
        knots(2, 0., 1.),
        knots(2, 0., 1.),
    )
    .unwrap();
    let u = knots(5, 0., 1.);
    let v = knots(3, 0., 1.);
    let result = try_fit_surface_in_basis(&original, [5, 3], [&u, &v], 0.)
        .unwrap()
        .unwrap();
    assert_eq!(result.maximum_deviation_bound, 0.);
    assert!(
        result
            .surface
            .control_points()
            .iter()
            .all(|p| p.point() == original.control_points()[0].point())
    );
}

#[test]
fn batched_solves_and_failures_preserve_inputs_and_validate_backend_output() {
    let original = source(false, 0.);
    let before = original.clone();
    let u = knots(3, -4., 2.);
    let v = knots(4, 3., 11.);
    let mut jobs = Vec::new();
    let result =
        try_interpolate_surface_in_basis_with_solver(&original, [3, 4], [&u, &v], &mut |job| {
            jobs.push((job.parameters.len(), job.columns));
            crate::surface_rebuild::portable_solve(job)
        })
        .unwrap();
    assert_eq!(jobs, vec![(4, 15), (5, 12)]);
    for axis in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
        for t in [0., 1.] {
            let x = if axis == SurfaceKnotDirection::U {
                t
            } else {
                1. - t
            };
            let y = if axis == SurfaceKnotDirection::U {
                t
            } else {
                t
            };
            assert_eq!(
                result
                    .evaluate(
                        result.parameter_at_u(x).unwrap(),
                        result.parameter_at_v(y).unwrap()
                    )
                    .unwrap(),
                original
                    .evaluate(
                        original.parameter_at_u(x).unwrap(),
                        original.parameter_at_v(y).unwrap()
                    )
                    .unwrap()
            );
        }
    }
    for failure in 0..4 {
        let mut calls = 0;
        let error = try_interpolate_surface_in_basis_with_solver(
            &original,
            [3, 4],
            [&u, &v],
            &mut |job| -> Result<Vec<Real>, GeometryError> {
                calls += 1;
                if calls == 2 {
                    match failure {
                        0 => return Ok(vec![]),
                        1 => return Ok(vec![Real::NAN; job.targets.len()]),
                        2 => return Ok(vec![Real::INFINITY; job.targets.len()]),
                        _ => return Err(GeometryError::SingularSystem),
                    }
                }
                crate::surface_rebuild::portable_solve(job)
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            GeometryError::InterpolationBackendFailed { .. } | GeometryError::SingularSystem
        ));
        assert_eq!(calls, 2);
        assert_eq!(original, before);
    }
}

#[test]
fn invalid_limits_and_bases_are_rejected_before_backend_work() {
    let original = source(false, 0.);
    let u = knots(3, -4., 2.);
    let v = knots(3, 3., 11.);
    let mut backend = |_: &NurbsInterpolationSystem<'_>| -> Result<Vec<Real>, GeometryError> {
        panic!("invalid input reached backend")
    };
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            try_fit_surface_in_basis_with_solver(&original, [3, 3], [&u, &v], limit, &mut backend),
            Err(GeometryError::InvalidTolerance)
        ));
    }
    for (degree, ku, kv) in [
        ([0, 3], u.clone(), v.clone()),
        ([12, 3], knots(12, -4., 2.), v.clone()),
        ([3, 3], u.clone(), knots(3, 0., 1.)),
        ([3, 3], u.clone(), vec![]),
        (
            [3, 3],
            u.clone(),
            vec![3., 3., 3., 3., 7., 7., 7., 7., 11., 11., 11., 11.],
        ),
        ([3, 3], knots(64, -4., 2.), v.clone()),
    ] {
        assert!(
            try_interpolate_surface_in_basis_with_solver(
                &original,
                degree,
                [&ku, &kv],
                &mut backend
            )
            .is_err()
        );
    }
}

#[test]
fn nonuniform_c0_bases_are_valid_and_a_finite_wrong_backend_cannot_bypass_the_bound() {
    let original = source(false, 0.);
    let reproduced = try_fit_surface_in_basis(
        &original,
        [original.degree_u(), original.degree_v()],
        [original.knots_u(), original.knots_v()],
        1e-10,
    )
    .unwrap()
    .unwrap();
    assert!(
        original
            .parameter_surface_deviation_bound(&reproduced.surface, 1e-10)
            .unwrap()
            .is_some()
    );
    let u = knots(3, -4., 2.);
    let v = knots(3, 3., 11.);
    let mut count = 0;
    let declined = try_fit_surface_in_basis_with_solver(
        &original,
        [3, 3],
        [&u, &v],
        1e-6,
        &mut |job| -> Result<Vec<Real>, GeometryError> {
            count += 1;
            Ok(vec![0.; job.targets.len()])
        },
    )
    .unwrap();
    assert_eq!(count, 2);
    assert!(declined.is_none());
}

#[test]
fn unsupported_source_weights_and_large_nets_fail_before_sampling_or_solving() {
    let original = source(false, 0.);
    let u = knots(3, -4., 2.);
    let v = knots(3, 3., 11.);
    let mut backend = |_: &NurbsInterpolationSystem<'_>| -> Result<Vec<Real>, GeometryError> {
        panic!("unsupported source reached backend")
    };
    for weight in [-1., -2.] {
        let mut controls = original.control_points().to_vec();
        controls[0] = WeightedPoint3::try_new(controls[0].point(), weight).unwrap();
        let unsupported = NurbsSurface::try_new_rational(
            original.degree_u(),
            original.degree_v(),
            original.control_point_count_u(),
            original.control_point_count_v(),
            controls,
            original.knots_u().to_vec(),
            original.knots_v().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            try_interpolate_surface_in_basis_with_solver(
                &unsupported,
                [3, 3],
                [&u, &v],
                &mut backend
            ),
            Err(GeometryError::UnsupportedSurfaceApproximation { .. })
        ));
    }
    let linear_knots = |n: usize| {
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
        vec![Point3::try_new(0., 0., 0.).unwrap(); 65 * 64],
        linear_knots(65),
        linear_knots(64),
    )
    .unwrap();
    assert!(matches!(
        try_interpolate_surface_in_basis_with_solver(
            &large,
            [1, 1],
            [&knots(1, 0., 1.), &knots(1, 0., 1.)],
            &mut backend
        ),
        Err(GeometryError::SurfaceApproximationLimit)
    ));
}
