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
            1e-10,
            retained.to_radians(),
        )
        .unwrap();
        assert_eq!(fit[0].control_points().len(), 7);
        assert!(
            try_prepare_refitted_loft_sections_with_angle(
                std::slice::from_ref(&source),
                1e-10,
                suppressed.to_radians(),
            )
            .is_err()
        );
        assert_eq!(source, before);
    }
    // An explicit zero does not remove the native one-degree resolution floor.
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
