use super::*;
use crate::Point3;

fn curve(points: &[[f64; 3]], weights: &[f64], degree: usize, knots: Vec<f64>) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        degree,
        points
            .iter()
            .zip(weights)
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(*p).unwrap(), *w).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}

fn uneven(weights: &[f64]) -> NurbsCurve {
    curve(
        &[[1., -2., 3.], [1.25, -1.5, 3.], [2., 0., 3.], [5., 6., 3.]],
        weights,
        3,
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
}

#[test]
fn rational_lines_have_uniform_cubic_speed_and_preserve_sources() {
    for weights in [[1., 2., 0.5, 1.], [-1., -2., -0.5, -1.]] {
        let source = uneven(&weights);
        let before = source.clone();
        let fitted =
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 1e-7).unwrap();
        let fitted = &fitted[0];
        assert_eq!(fitted.degree(), 3);
        assert_eq!(fitted.control_points().len(), 4);
        assert!(fitted.control_points().iter().all(|c| c.weight() == 1.));
        for i in 0..=20 {
            let t = f64::from(i) / 20.;
            let actual = fitted.evaluate(t).unwrap();
            assert!(
                actual
                    .distance_to(Point3::try_new(1. + 4. * t, -2. + 8. * t, 3.).unwrap())
                    .unwrap()
                    < 4e-15
            );
        }
        assert!(
            source
                .evaluate(0.5)
                .unwrap()
                .distance_to(fitted.evaluate(0.5).unwrap())
                .unwrap()
                > 1.
        );
        assert_eq!(source, before);
    }
}

#[test]
fn continuous_piecewise_lines_and_stationary_ends_are_admitted() {
    let source = curve(
        &[[0., 0., 0.], [1., 0., 0.], [4., 0., 0.]],
        &[1.; 3],
        1,
        vec![-7., -7., 0.3, 13., 13.],
    );
    let fitted = try_prepare_refitted_loft_sections(&[source], 1e-7).unwrap();
    assert_eq!(fitted[0].evaluate(0.5).unwrap().x(), 2.);
    let source = curve(
        &[[0., 0., 0.], [0., 0., 0.], [4., 0., 0.], [4., 0., 0.]],
        &[1.; 4],
        3,
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    );
    assert!(try_prepare_refitted_loft_sections(&[source], 1e-7).is_ok());
}

#[test]
fn unclamped_rational_lines_use_active_endpoints_with_one_final_rounding() {
    for weights in [[1., 2., 0.5], [-1., -2., -0.5]] {
        let source = curve(
            &[[0., 0., 0.], [1., 0.5, 0.], [4., 2., 0.]],
            &weights,
            2,
            vec![-13., -13., -3., 7., 17., 17.],
        );
        let before = source.clone();
        let fit = straight(&source, 1e-12).unwrap().curve;
        for i in 0..=16 {
            let t = f64::from(i) / 16.;
            let x = (1. - t) * (2. / 3.) + t * (8. / 5.);
            assert!(
                fit.evaluate(t)
                    .unwrap()
                    .distance_to(Point3::try_new(x, x / 2., 0.).unwrap())
                    .unwrap()
                    < 1e-15
            );
        }
        assert_eq!(source, before);
        assert!(straight(&source, 1e-18).is_err());
        assert_eq!(
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 0.).unwrap(),
            [source]
        );
    }
}

#[test]
fn inactive_outer_controls_do_not_define_the_fitted_segment() {
    let source = curve(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
        ],
        &[1.; 5],
        2,
        vec![-1., 0., 0., 0., 1., 1., 1., 2.],
    );
    assert!(matches!(
        straight(&source, 1e-9).map(|s| s.curve),
        Err(GeometryError::UnsupportedCurveApproximation { .. })
    ));
}

#[test]
fn curves_reversals_jumps_and_mixed_weights_fail_explicitly() {
    let source = uneven(&[1.; 4]);
    let mut near = source
        .control_points()
        .iter()
        .map(|c| c.point().to_array())
        .collect::<Vec<_>>();
    near[1][2] += 1e-12;
    let knots = source.knots().to_vec();
    let mut reverse = near.clone();
    reverse[1] = [4., 4., 3.];
    reverse[2] = [2., 0., 3.];
    for source in [
        curve(&near, &[1.; 4], 3, knots.clone()),
        curve(&reverse, &[1.; 4], 3, knots),
        uneven(&[1., -2., 0.5, 1.]),
        curve(
            &[[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [3., 0., 0.]],
            &[1.; 4],
            1,
            vec![0., 0., 0.5, 0.5, 1., 1.],
        ),
        curve(
            &[[0., 0., 0.]; 4],
            &[1.; 4],
            3,
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        ),
    ] {
        assert!(matches!(
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 0.1),
            Err(GeometryError::UnsupportedCurveApproximation { .. })
        ));
        assert_eq!(
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 0.).unwrap(),
            [source]
        );
    }
}

#[test]
fn unrepresentable_handle_tolerances_and_resource_overflows_are_rejected() {
    let line = curve(
        &[[0., 0., 0.], [2., 0., 0.]],
        &[1.; 2],
        1,
        vec![0., 0., 1., 1.],
    );
    assert!(straight(&line, 1e-300).is_err());
    for tolerance in [-1., f64::NAN, f64::INFINITY] {
        assert_eq!(
            try_prepare_refitted_loft_sections(&[], tolerance),
            Err(GeometryError::InvalidTolerance)
        );
    }
    assert!(matches!(
        try_prepare_refitted_loft_sections(&vec![line; 257], 0.001),
        Err(GeometryError::LoftResourceLimit { maximum: 256, .. })
    ));
    let degree = 100;
    let source = NurbsCurve::try_new(
        degree,
        (0..=degree)
            .map(|i| Point3::try_new(i as f64, 0., 0.).unwrap())
            .collect(),
        (0..2 * (degree + 1)).map(|i| i as f64).collect(),
    )
    .unwrap();
    assert_eq!(
        straight(&source, 1e-9).map(|s| s.curve),
        Err(GeometryError::SurfaceCurveCertificateWorkLimit)
    );
}

#[test]
fn disabled_fitting_preserves_definitions_through_the_inclusive_native_floor() {
    let source = uneven(&[1., 2., 0.5, 1.]);
    let floor = 2. * Real::EPSILON.sqrt();
    for limit in [0., 1e-300, floor.next_down(), floor] {
        assert_eq!(
            try_prepare_refitted_loft_sections(std::slice::from_ref(&source), limit).unwrap(),
            std::slice::from_ref(&source)
        );
        // The callback still runs: disabling fitting does not disable the
        // shared-basis preparation needed by a surface backend.
        let mut clamped = false;
        let prepared = prepare_refitted_loft_sections_with_operations::<GeometryError>(
            std::slice::from_ref(&source),
            limit,
            None,
            &mut |c| {
                clamped = true;
                c.clamped_to_active_domain()
            },
            &mut |c, d| c.try_change_degree(d, false),
            &mut |c, k, m| c.try_insert_knot(k, m),
        )
        .unwrap();
        assert!(clamped);
        assert_eq!(prepared.len(), 1);
    }
    let fitted =
        try_prepare_refitted_loft_sections(std::slice::from_ref(&source), floor.next_up()).unwrap();
    assert_ne!(fitted.as_slice(), std::slice::from_ref(&source));
    assert!(matches!(
        try_prepare_refitted_loft_sections_with_angle(&[source], floor, Real::NAN),
        Err(GeometryError::InvalidContinuityAngle)
    ));
}
