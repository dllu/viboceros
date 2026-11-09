use super::*;
use crate::WeightedPoint3;

fn knots(p: usize) -> Vec<Real> {
    [vec![0.; p + 1], vec![1.; p + 1]].concat()
}
fn rational_strip() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        3,
        1,
        4,
        2,
        (0..2)
            .flat_map(|j| {
                (0..4).map(move |i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(i as Real / 3., j as Real, (i * i) as Real / 9.).unwrap(),
                        [1., 0.5, 2., 1.][i],
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots(3),
        knots(1),
    )
    .unwrap()
}

#[test]
fn rational_strip_refines_only_the_direction_that_needs_fitting() {
    let source = rational_strip();
    let original = source.clone();
    let options = SurfaceFitOptions {
        degree: [3, 1],
        maximum_control_count: [32, 2],
    };
    let result = try_fit_surface_with_bound(&source, options, 0.002)
        .unwrap()
        .unwrap();
    assert!(result.surface.control_point_count_u() > 4);
    assert_eq!(result.surface.control_point_count_v(), 2);
    assert!(result.maximum_deviation_bound <= 0.002);
    assert!(
        source
            .parameter_surface_deviation_bound(&result.surface, 0.002)
            .unwrap()
            .is_some()
    );
    assert_eq!(source, original);
}

#[test]
fn fitting_reduces_a_redundant_polynomial_and_changes_degrees() {
    let base = NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|j| {
                (0..3).map(move |i| {
                    Point3::try_new(i as Real / 2., j as Real / 2., (i * j) as Real / 4.).unwrap()
                })
            })
            .collect(),
        knots(2),
        knots(2),
    )
    .unwrap();
    let source = base
        .try_insert_knot_u(0.25, 2)
        .unwrap()
        .try_insert_knot_u(0.75, 2)
        .unwrap()
        .try_insert_knot_v(0.5, 2)
        .unwrap();
    let result = try_fit_surface_with_bound(&source, SurfaceFitOptions::default(), 1e-10)
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            result.surface.control_point_count_u(),
            result.surface.control_point_count_v()
        ),
        (4, 4)
    );
    assert!(
        base.parameter_surface_deviation_bound(&result.surface, 1e-10)
            .unwrap()
            .is_some()
    );
}

#[test]
fn hidden_peak_requires_refinement_and_is_never_accepted_from_samples_alone() {
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
        knots(6),
        knots(1),
    )
    .unwrap();
    let small = SurfaceFitOptions {
        degree: [1, 1],
        maximum_control_count: [2, 2],
    };
    assert!(
        try_fit_surface_with_bound(&source, small, 0.01)
            .unwrap()
            .is_none()
    );
    let result = try_fit_surface_with_bound(
        &source,
        SurfaceFitOptions {
            maximum_control_count: [32, 2],
            ..small
        },
        0.01,
    )
    .unwrap()
    .unwrap();
    assert!(result.surface.control_point_count_u() > 2);
    assert!(
        source
            .parameter_surface_deviation_bound(&result.surface, 0.01)
            .unwrap()
            .is_some()
    );
}

#[test]
fn exact_zero_accepts_constant_tensors_and_budget_exhaustion_preserves_the_source() {
    let source = NurbsSurface::try_new(
        1,
        1,
        2,
        2,
        vec![Point3::try_new(3., 4., 5.).unwrap(); 4],
        knots(1),
        knots(1),
    )
    .unwrap();
    let result = try_fit_surface_with_bound(&source, SurfaceFitOptions::default(), 0.)
        .unwrap()
        .unwrap();
    assert_eq!(result.maximum_deviation_bound, 0.);
    let curved = rational_strip();
    let before = curved.clone();
    assert!(
        try_fit_surface_with_bound(
            &curved,
            SurfaceFitOptions {
                degree: [3, 1],
                maximum_control_count: [4, 2]
            },
            1e-10
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(curved, before);
}

#[test]
fn invalid_options_fail_before_backend_work() {
    let source = rational_strip();
    let mut solve = |_: &NurbsInterpolationSystem<'_>| -> Result<Vec<Real>, GeometryError> {
        panic!("invalid request reached solver")
    };
    for degree in [[0, 3], [3, 12], [4, 3]] {
        assert!(
            try_fit_surface_with_bound_with_solver(
                &source,
                SurfaceFitOptions {
                    degree,
                    maximum_control_count: [4, 4]
                },
                0.01,
                &mut solve
            )
            .is_err()
        );
    }
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert!(matches!(
            try_fit_surface_with_bound_with_solver(
                &source,
                SurfaceFitOptions::default(),
                limit,
                &mut solve
            ),
            Err(GeometryError::InvalidTolerance)
        ));
    }
    assert!(
        try_fit_surface_with_bound_with_solver(
            &source,
            SurfaceFitOptions {
                maximum_control_count: [65, 4],
                ..Default::default()
            },
            0.01,
            &mut solve
        )
        .is_err()
    );
}

#[test]
fn backend_failure_after_initial_proposal_exposes_no_partial_surface() {
    let source = rational_strip();
    let before = source.clone();
    let mut calls = 0;
    let result = try_fit_surface_with_bound_with_solver(
        &source,
        SurfaceFitOptions {
            degree: [3, 1],
            maximum_control_count: [16, 2],
        },
        0.001,
        &mut |job| -> Result<Vec<Real>, GeometryError> {
            calls += 1;
            if calls == 3 {
                Err(GeometryError::SingularSystem)
            } else {
                crate::surface_rebuild::portable_solve(job)
            }
        },
    );
    assert!(matches!(result, Err(GeometryError::SingularSystem)));
    assert_eq!(calls, 3);
    assert_eq!(source, before);
}

#[test]
fn an_arithmetic_or_sampling_limit_is_not_an_accuracy_claim() {
    let source = rational_strip();
    let options = SurfaceFitOptions {
        degree: [3, 1],
        maximum_control_count: [8, 2],
    };
    let mut cache = HashMap::new();
    let candidate =
        try_interpolate_surface_in_basis(&source, [3, 1], [&knots(3), &knots(1)]).unwrap();
    let mut cuts = [vec![0., 1.], vec![0., 1.]];
    let error = refine::refine(&candidate, &mut cuts, options, 0.001, &mut |u, v| {
        cache.insert((u.to_bits(), v.to_bits()), ());
        if cache.len() > 3 {
            Err(GeometryError::SurfaceApproximationLimit)
        } else {
            source.evaluate(u, v)
        }
    })
    .unwrap_err();
    assert!(matches!(error, GeometryError::SurfaceApproximationLimit));
    assert_eq!(cuts, [vec![0., 1.], vec![0., 1.]]);
}

#[test]
fn a_peak_missing_every_initial_station_still_triggers_another_proposal() {
    // Bernstein coefficients of 1e5*u*(u-1)*(u-1/4)^2*(u-1/3)^2*
    // (u-1/2)^2*(u-2/3)^2*(u-3/4)^2. Every initial Greville and refinement
    // station is a root, while the surface has a large excursion between them.
    let z = [
        0.,
        -3.6168981481481484,
        8.987443883277217,
        -15.591023078002245,
        22.207998191794488,
        -27.1800153346739,
        29.03434978087756,
        -27.1800153346739,
        22.207998191794488,
        -15.591023078002245,
        8.987443883277217,
        -3.6168981481481484,
        0.,
    ];
    let source = NurbsSurface::try_new(
        12,
        1,
        13,
        2,
        (0..2)
            .flat_map(|j| {
                z.iter()
                    .enumerate()
                    .map(move |(i, &z)| Point3::try_new(i as Real / 12., j as Real, z).unwrap())
            })
            .collect(),
        knots(12),
        knots(1),
    )
    .unwrap();
    for u in [0., 0.25, 1. / 3., 0.5, 2. / 3., 0.75, 1.] {
        assert!(source.evaluate(u, 0.).unwrap().z().abs() < 1e-9);
    }
    assert!(source.evaluate(0.04, 0.).unwrap().z().abs() > 0.1);
    let mut calls = 0;
    let result = try_fit_surface_with_bound_with_solver(
        &source,
        SurfaceFitOptions {
            degree: [3, 1],
            maximum_control_count: [8, 2],
        },
        0.001,
        &mut |job| -> Result<Vec<Real>, GeometryError> {
            calls += 1;
            if calls == 3 {
                Err(GeometryError::SingularSystem)
            } else {
                crate::surface_rebuild::portable_solve(job)
            }
        },
    );
    assert!(matches!(result, Err(GeometryError::SingularSystem)));
    assert_eq!(calls, 3);
}
