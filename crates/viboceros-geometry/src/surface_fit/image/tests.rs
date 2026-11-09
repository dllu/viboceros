use super::*;
use crate::{AffineTransform3, Vector3, WeightedPoint3};

fn source(x: &[Real], z: impl Fn(usize, usize) -> Real) -> NurbsSurface {
    let n = x.len();
    NurbsSurface::try_new_rational(
        n - 1,
        2,
        n,
        3,
        (0..3)
            .flat_map(|j| {
                let z = &z;
                x.iter().enumerate().map(move |(i, &x)| {
                    WeightedPoint3::try_new(
                        Point3::try_new(x, j as Real / 2., z(i, j)).unwrap(),
                        1. + (i % 2) as Real,
                    )
                    .unwrap()
                })
            })
            .collect(),
        [vec![0.; n], vec![1.; n]].concat(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap()
}
fn options() -> SurfaceFitOptions {
    SurfaceFitOptions {
        degree: [1, 1],
        maximum_control_count: [2, 2],
    }
}

#[test]
fn planar_rational_speed_changes_fit_without_changing_the_uv_contract() {
    let source = source(&[0., 0.05, 0.8, 1.], |_, _| 0.)
        .try_reparameterized(-3. ..=2., 7. ..=12.)
        .unwrap();
    let before = source.clone();
    assert!(
        try_fit_surface_with_bound(&source, options(), 0.)
            .unwrap()
            .is_none()
    );
    let mut calls = 0;
    let fit = try_fit_surface_image_with_bound_with_solver(&source, options(), 0., &mut |job| {
        calls += 1;
        crate::surface_rebuild::portable_solve(job)
    })
    .unwrap()
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(fit.maximum_deviation_bound, 0.);
    assert_eq!(
        (
            fit.surface.control_point_count_u(),
            fit.surface.control_point_count_v()
        ),
        (2, 2)
    );
    assert_eq!(fit.surface.domain_u(), source.domain_u());
    assert_eq!(fit.surface.domain_v(), source.domain_v());
    assert!(
        source
            .parameter_surface_deviation_bound(&fit.surface, 1e-9)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        source
            .planar_surface_image_deviation_bound(&fit.surface, 0.)
            .unwrap(),
        Some(0.)
    );
    assert_eq!(source, before);
}

#[test]
fn a_fold_with_an_outside_control_hull_still_has_a_complete_image_fit() {
    let source = source(&[0., 1.1, -0.1, 1.], |_, _| 0.);
    assert_eq!(
        try_fit_surface_image_with_bound(&source, options(), 0.)
            .unwrap()
            .unwrap()
            .maximum_deviation_bound,
        0.
    );
    let outside = source
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(0., 0., 0.125).unwrap(),
        ))
        .unwrap();
    let result = try_fit_surface_image_with_bound(&outside, options(), 0.)
        .unwrap()
        .unwrap();
    assert_eq!(result.surface.control_points()[0].point().z(), 0.125);
}

#[test]
fn hidden_interior_features_and_real_overshoot_cannot_be_simplified_at_zero() {
    for source in [
        source(&[0., 2., 2., 1.], |_, _| 0.),
        source(
            &[0., 0.5, 1.],
            |i, j| if i == 1 && j == 1 { 1. } else { 0. },
        ),
    ] {
        let before = source.clone();
        assert!(
            try_fit_surface_image_with_bound(&source, options(), 0.)
                .unwrap()
                .is_none()
        );
        assert!(
            try_fit_surface_image_with_bound(&source, options(), 0.01)
                .unwrap()
                .is_none()
        );
        assert_eq!(source, before);
    }
}

#[test]
fn a_bowed_boundary_gets_a_conservative_image_bound_and_a_sharp_gate() {
    let source = source(&[0., 0.05, 0.8, 1.], |i, j| {
        if i == 1 && j == 0 { 0.125 } else { 0. }
    });
    let result = try_fit_surface_image_with_bound(&source, options(), 0.125)
        .unwrap()
        .unwrap();
    assert_eq!(result.maximum_deviation_bound, 0.125);
    assert!(
        source
            .parameter_surface_deviation_bound(&result.surface, 0.125)
            .unwrap()
            .is_none()
    );
    assert!(
        try_fit_surface_image_with_bound(&source, options(), 0.125_f64.next_down())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_successful_initial_parameter_fit_retains_its_controls_and_backend_work() {
    let source = source(&[0., 0.05, 0.8, 1.], |i, j| (i * j) as Real / 8.);
    let options = SurfaceFitOptions {
        degree: [3, 2],
        maximum_control_count: [4, 3],
    };
    let mut calls = 0;
    let fit = try_fit_surface_image_with_bound_with_solver(&source, options, 1., &mut |job| {
        calls += 1;
        crate::surface_rebuild::portable_solve(job)
    })
    .unwrap()
    .unwrap();
    let ordinary = try_fit_surface_with_bound(&source, options, 1.)
        .unwrap()
        .unwrap();
    assert_eq!(fit.surface, ordinary.surface);
    assert_eq!(
        fit.maximum_deviation_bound,
        ordinary.maximum_deviation_bound
    );
    assert_eq!(calls, 2);
}

#[test]
fn validation_and_backend_failure_precede_any_analytic_proposal() {
    let source = source(&[0., 0.05, 0.8, 1.], |_, _| 0.);
    let before = source.clone();
    let mut calls = 0;
    let mut solve = |_: &NurbsInterpolationSystem<'_>| -> Result<Vec<Real>, GeometryError> {
        calls += 1;
        Err(GeometryError::SingularSystem)
    };
    for limit in [-1., Real::NAN, Real::INFINITY] {
        assert!(
            try_fit_surface_image_with_bound_with_solver(&source, options(), limit, &mut solve)
                .is_err()
        );
    }
    let invalid = SurfaceFitOptions {
        degree: [0, 1],
        ..options()
    };
    assert!(
        try_fit_surface_image_with_bound_with_solver(&source, invalid, 0., &mut solve).is_err()
    );
    assert!(matches!(
        try_fit_surface_image_with_bound_with_solver(&source, options(), 0., &mut solve),
        Err(GeometryError::SingularSystem)
    ));
    assert_eq!(calls, 1);
    assert_eq!(source, before);
}

#[test]
fn a_declined_quad_uses_the_existing_adaptive_parameter_fitter() {
    let source = source(&[0., 0.1, 0.8, 1.], |i, j| (i * j) as Real / 6.);
    let options = SurfaceFitOptions {
        degree: [3, 2],
        maximum_control_count: [16, 16],
    };
    let ordinary = try_fit_surface_with_bound(&source, options, 0.002)
        .unwrap()
        .unwrap();
    let fit = try_fit_surface_image_with_bound(&source, options, 0.002)
        .unwrap()
        .unwrap();
    assert_eq!(fit.surface, ordinary.surface);
    assert_eq!(
        fit.maximum_deviation_bound,
        ordinary.maximum_deviation_bound
    );
}

#[test]
fn discontinuous_sources_cannot_use_stored_corners_to_bridge_a_gap() {
    let source = NurbsSurface::try_new(
        1,
        1,
        4,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                [0., 0.2, 0.8, 1.]
                    .into_iter()
                    .map(move |x| Point3::try_new(x, y, 0.).unwrap())
            })
            .collect(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(
        try_fit_surface_image_with_bound(&source, options(), 0.)
            .unwrap()
            .is_none()
    );
}

#[test]
fn unclamped_corners_decline_and_common_negative_gauges_remain_valid() {
    let base = source(&[0., 0.05, 0.8, 1.], |_, _| 0.);
    let unclamped = NurbsSurface::try_new_rational(
        3,
        2,
        4,
        3,
        base.control_points().to_vec(),
        vec![-3., -2., -1., 0., 1., 2., 3., 4.],
        base.knots_v().to_vec(),
    )
    .unwrap();
    assert!(
        try_fit_surface_image_with_bound(&unclamped, options(), 0.)
            .unwrap()
            .is_none()
    );
    let negative = NurbsSurface::try_new_rational(
        3,
        2,
        4,
        3,
        base.control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), -p.weight() * 8.).unwrap())
            .collect(),
        base.knots_u().to_vec(),
        base.knots_v().to_vec(),
    )
    .unwrap();
    let result = try_fit_surface_image_with_bound(&negative, options(), 0.)
        .unwrap()
        .unwrap();
    assert_eq!(result.maximum_deviation_bound, 0.);
}

#[test]
fn tilted_vertical_and_translated_planes_have_the_same_complete_image_contract() {
    let base = source(&[0., 0.125, 0.75, 1.], |_, _| 0.);
    for frame in 0..3 {
        let controls = base
            .control_points()
            .iter()
            .map(|p| {
                let q = p.point();
                let q = match frame {
                    0 => Point3::try_new(q.x(), q.y(), q.x() + 2. * q.y()).unwrap(),
                    1 => Point3::try_new(0., q.y(), q.x()).unwrap(),
                    _ => Point3::try_new(q.x(), q.y(), 1e12).unwrap(),
                };
                WeightedPoint3::try_new(q, p.weight()).unwrap()
            })
            .collect();
        let source = NurbsSurface::try_new_rational(
            3,
            2,
            4,
            3,
            controls,
            base.knots_u().to_vec(),
            base.knots_v().to_vec(),
        )
        .unwrap();
        let fit = try_fit_surface_image_with_bound(&source, options(), 0.)
            .unwrap()
            .unwrap();
        assert_eq!(fit.maximum_deviation_bound, 0.);
        assert!(
            source
                .parameter_surface_deviation_bound(&fit.surface, 1e-9)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            source
                .planar_surface_image_deviation_bound(&fit.surface, 0.)
                .unwrap(),
            Some(0.)
        );
    }
}
