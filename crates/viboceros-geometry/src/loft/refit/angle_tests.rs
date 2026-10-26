use super::*;
use crate::Point3;

fn bent(degrees: Real) -> NurbsCurve {
    let angle = degrees.to_radians();
    NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(1. + angle.cos(), angle.sin(), 0.).unwrap(),
        ],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap()
}

#[test]
fn requested_angles_resolve_acute_orthogonal_obtuse_and_reversing_corners() {
    for (bend, retained, suppressed) in [
        (30_f64, 29_f64, 30_f64),
        (90., 90., 91.),
        (150., 149., 150.),
        (180., 179., 180.),
    ] {
        let source = if bend == 90. {
            NurbsCurve::try_new(
                1,
                vec![
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(1., 0., 0.).unwrap(),
                    Point3::try_new(1., 1., 0.).unwrap(),
                ],
                vec![0., 0., 1., 2., 2.],
            )
            .unwrap()
        } else {
            bent(bend)
        };
        let before = source.clone();
        let fit = try_prepare_refitted_loft_sections_with_angle(
            std::slice::from_ref(&source),
            1e-7,
            retained.to_radians(),
        )
        .unwrap();
        assert_eq!(fit[0].control_points().len(), 7);
        assert!(
            try_prepare_refitted_loft_sections_with_angle(
                std::slice::from_ref(&source),
                1e-7,
                suppressed.to_radians(),
            )
            .is_err()
        );
        assert_eq!(source, before);
    }
    // An explicit zero selects the native one-degree default.
    assert!(try_prepare_refitted_loft_sections_with_angle(&[bent(0.5)], 1e-3, 0.).is_err());
}

#[test]
fn invalid_angles_stop_before_backend_edits_even_when_fitting_is_disabled() {
    for angle in [
        -1.,
        Real::NAN,
        Real::INFINITY,
        std::f64::consts::PI + Real::EPSILON * 4.,
    ] {
        for fit in [0., 1e-3] {
            let result: Result<Vec<NurbsCurve>, GeometryError> =
                prepare_refitted_loft_sections_with_operations(
                    &[bent(90.)],
                    fit,
                    Some(angle),
                    &mut |_| panic!("invalid angle reached clamping"),
                    &mut |_, _| panic!("invalid angle reached elevation"),
                    &mut |_, _, _| panic!("invalid angle reached refinement"),
                );
            assert!(matches!(result, Err(GeometryError::InvalidContinuityAngle)));
        }
    }
}

#[test]
fn positive_angles_use_the_minimum_while_zero_and_default_keep_one_degree() {
    for bend in [0.05_f64, 0.1, 0.10001, 0.11458, 0.11460, 0.15, 0.25, 0.9] {
        let source = bent(bend);
        let before = source.clone();
        for angle in [1e-8, 0.05_f64.to_radians(), 0.1_f64.to_radians()] {
            let result = try_prepare_refitted_loft_sections_with_angle(
                std::slice::from_ref(&source),
                1e-7,
                angle,
            );
            assert_eq!(
                result.is_ok(),
                bend.to_radians() > 0.002,
                "bend={bend} angle={angle}"
            );
            if let Ok(curves) = result {
                let fit = &curves[0];
                assert_eq!(fit.control_points().len(), 7);
                let fraction = fit.knots()[4] / fit.domain().end();
                assert!(
                    source
                        .mapped_curve_deviation_bound(
                            fit,
                            &[[0., 0.], [0.5, fraction], [1., 1.]],
                            1e-10
                        )
                        .unwrap()
                        .is_some()
                );
            }
        }
        assert!(try_prepare_refitted_loft_sections(std::slice::from_ref(&source), 1e-7).is_err());
        assert!(
            try_prepare_refitted_loft_sections_with_angle(std::slice::from_ref(&source), 1e-7, 0.)
                .is_err()
        );
        assert_eq!(source, before);
    }
}

#[test]
fn suppressed_small_angles_cannot_bypass_corner_policy_through_smooth_fitting() {
    // These two cubic spans have a resolved quarter-degree tangent corner but
    // a coarse positional tolerance could otherwise admit one smooth cubic.
    let a = 0.25_f64.to_radians();
    let source = NurbsCurve::try_new(
        3,
        [
            [0., 0., 0.],
            [0.2, 0., 0.],
            [0.8, 0., 0.],
            [1., 0., 0.],
            [1. + 0.2 * a.cos(), 0.2 * a.sin(), 0.],
            [1. + 0.8 * a.cos(), 0.8 * a.sin(), 0.],
            [1. + a.cos(), a.sin(), 0.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        [vec![0.; 4], vec![0.5; 3], vec![1.; 4]].concat(),
    )
    .unwrap();
    assert!(candidate::prepare(&[source.clone(), source], 0.1, Some(1e-8)).is_err());
}

#[test]
fn explicit_suppression_allows_certified_coarse_initial_cubics_above_one_degree() {
    for degrees in [2_f64, 10.] {
        let source = bent(degrees);
        let inputs = vec![source.clone(); 2];
        let fitted = try_prepare_refitted_loft_sections_with_angle(
            &inputs,
            0.1,
            (degrees * 1.5).to_radians(),
        )
        .unwrap();
        for curve in fitted {
            assert_eq!(curve.control_points().len(), 4);
            assert!(
                source
                    .parameter_curve_deviation_bound(&curve, 0.05)
                    .unwrap()
                    .is_some()
            );
        }
        assert!(inputs.iter().all(|c| c == &source));
    }
}
