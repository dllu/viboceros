use super::*;
use crate::{AffineTransform3, Circle3, Curve3, NurbsCurve, Point3, Polyline3};
fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn profile() -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        vec![
            point(0., 0., 0.),
            point(3., 4., 0.),
            point(7., -2., 0.),
            point(10., 0., 0.),
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}
#[test]
fn crv_overlap_transverse_ends_survive_curve_shape_parameter_domain_and_reversal() {
    let a = profile();
    let b = a
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(0., 1., 2.).unwrap(),
        ))
        .unwrap()
        .try_reparameterized(100. ..=300.)
        .unwrap();
    for reverse in [false, true] {
        let b = if reverse {
            b.reversed().unwrap()
        } else {
            b.clone()
        };
        assert_eq!(
            preserve_transverse_curve_ends(
                CurveRef::NurbsCurve(&a),
                CurveRef::NurbsCurve(&b),
                [[0.1, 0.9], [0.2, 0.8]],
                Tolerance::DEFAULT
            )
            .unwrap(),
            [[0., 1.]; 2]
        );
    }
}
#[test]
fn crv_overlap_only_transverse_boundary_expands_and_longitudinal_offsets_stay_projected() {
    let a = profile();
    let b = NurbsCurve::try_new(
        1,
        vec![point(3., 1., 0.), point(10., 1., 0.)],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert_eq!(
        preserve_transverse_curve_ends(
            CurveRef::NurbsCurve(&a),
            CurveRef::NurbsCurve(&b),
            [[0.3, 0.9], [0., 0.8]],
            Tolerance::DEFAULT
        )
        .unwrap(),
        [[0.3, 1.], [0., 1.]]
    );
    let c = b
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(2., 0., 0.).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        preserve_transverse_curve_ends(
            CurveRef::NurbsCurve(&a),
            CurveRef::NurbsCurve(&c),
            [[0.3, 0.9], [0., 0.8]],
            Tolerance::DEFAULT
        )
        .unwrap(),
        [[0.3, 0.9], [0., 0.8]]
    );
}
#[test]
fn crv_overlap_rotated_transverse_profiles_use_model_tolerance_without_changing_curves() {
    let a = Polyline3::try_new(
        vec![point(0., 0., 0.), point(4., 5., 1.), point(8., 6., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = a
        .transformed(
            AffineTransform3::from_translation(Vector3::try_new(-3., 4., 0.).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(
        preserve_transverse_curve_ends(
            CurveRef::Polyline(&a),
            CurveRef::Polyline(&b),
            [[0.1, 0.9]; 2],
            Tolerance::DEFAULT
        )
        .unwrap(),
        [[0., 1.]; 2]
    );
    let before = a.clone();
    let c = b
        .transformed(
            AffineTransform3::from_translation(Vector3::try_new(1e-5, 0., 0.).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert_eq!(
        preserve_transverse_curve_ends(
            CurveRef::Polyline(&a),
            CurveRef::Polyline(&c),
            [[0.1, 0.9]; 2],
            Tolerance::DEFAULT
        )
        .unwrap(),
        [[0.1, 0.9]; 2]
    );
    assert_eq!(a, before);
}
#[test]
fn crv_overlap_closed_inputs_and_invalid_windows_keep_distinct_outcomes() {
    let a = Circle3::try_new(
        point(0., 0., 0.),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let b = Curve3::NurbsCurve(profile());
    assert_eq!(
        preserve_transverse_curve_ends(
            CurveRef::Circle(&a),
            b.as_ref(),
            [[0.25, 1.], [0., 0.5]],
            Tolerance::DEFAULT
        )
        .unwrap(),
        [[0.25, 1.], [0., 0.5]]
    );
    for range in [[f64::NAN, 1.], [-0.1, 1.], [0., 1.1], [0.9, 0.2]] {
        assert!(
            preserve_transverse_curve_ends(
                CurveRef::Circle(&a),
                b.as_ref(),
                [[0., 1.], range],
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}
