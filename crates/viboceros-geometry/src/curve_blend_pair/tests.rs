use super::*;
use crate::ParameterSide;
use crate::{CircularArc3, CurveRef, LineSegment, Vector3};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn line(start: Point3, end: Point3) -> Curve3 {
    Curve3::Line(LineSegment::try_new(start, end, Tolerance::DEFAULT).unwrap())
}

#[test]
fn bad_interpolation_results_and_backend_errors_preserve_the_source_curves() {
    let first = line(p(0., 0.), p(1., 0.));
    let second = line(p(4., 1.), p(4., 2.));
    let before = [first.clone(), second.clone()];
    for failure in 0..4 {
        let mut interpolate = |input: &EndpointInterpolation<'_>| {
            if failure == 3 {
                return Err(GeometryError::InterpolationBackendFailed {
                    message: "injected failure".into(),
                });
            }
            let mut net = crate::try_interpolate_endpoints(input)?;
            match failure {
                0 => net.clear(),
                1 => net[1][3] = 0.5,
                2 => net[1][0] += 1.,
                _ => unreachable!(),
            }
            Ok(net)
        };
        assert!(
            try_blend_curve_with_interpolation(
                &first,
                p(1., 0.),
                &second,
                p(4., 1.),
                CurveBlendOptions::default(),
                Tolerance::DEFAULT,
                &mut interpolate
            )
            .is_err()
        );
        assert_eq!([first.clone(), second.clone()], before);
    }
}

#[test]
fn tangent_blend_matches_both_oriented_curve_ends() {
    let first = line(p(0., 0.), p(1., 0.));
    let second = line(p(4., 1.), p(4., 2.));
    let blend = try_blend_curve(
        &first,
        p(1., 0.),
        &second,
        p(4., 1.),
        CurveBlendOptions::default(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(blend.degree(), 3);
    let start = CurveRef::NurbsCurve(&blend)
        .evaluate_with_tangent_on_side(*blend.domain().start(), ParameterSide::Right)
        .unwrap();
    let end = CurveRef::NurbsCurve(&blend)
        .evaluate_with_tangent_on_side(*blend.domain().end(), ParameterSide::Left)
        .unwrap();
    assert!(start.point().distance_to(p(1., 0.)).unwrap() < 1e-12);
    assert!(end.point().distance_to(p(4., 1.)).unwrap() < 1e-12);
    assert!(
        start
            .tangent()
            .as_vector()
            .angle_to(Vector3::try_new(1., 0., 0.).unwrap())
            .unwrap()
            < 1e-12
    );
    assert!(
        end.tangent()
            .as_vector()
            .angle_to(Vector3::try_new(0., 1., 0.).unwrap())
            .unwrap()
            < 1e-12
    );
}

#[test]
fn reversed_inputs_still_make_an_outward_tangent_blend() {
    let first = line(p(1., 0.), p(0., 0.));
    let second = line(p(4., 2.), p(4., 1.));
    let blend = try_blend_curve(
        &first,
        p(1., 0.),
        &second,
        p(4., 1.),
        CurveBlendOptions {
            handles: [Some(0.5), Some(0.75)],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let controls = blend.control_points();
    assert!(controls[1].point().distance_to(p(1.5, 0.)).unwrap() < 1e-12);
    assert!(controls[2].point().distance_to(p(4., 0.25)).unwrap() < 1e-12);
}

#[test]
fn mixed_position_and_tangency_use_a_quadratic() {
    let first = line(p(0., -1.), p(0., 0.));
    let second = line(p(3., 1.), p(3., 2.));
    let blend = try_blend_curve(
        &first,
        p(0., 0.),
        &second,
        p(3., 1.),
        CurveBlendOptions {
            continuity: [
                CurveBlendContinuity::Position,
                CurveBlendContinuity::Tangency,
            ],
            handles: [Some(1.), Some(1.)],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let controls = blend.control_points();
    assert_eq!(blend.degree(), 2);
    assert_eq!(controls.len(), 3);
    assert!(controls[0].point().distance_to(p(0., 0.)).unwrap() < 1e-12);
    assert!(controls[1].point().distance_to(p(3., 0.)).unwrap() < 1e-12);
    assert!(controls[2].point().distance_to(p(3., 1.)).unwrap() < 1e-12);
}

#[test]
fn position_only_end_does_not_require_a_source_tangent() {
    let source = Curve3::NurbsCurve(
        NurbsCurve::try_new(
            1,
            vec![p(0., 0.), p(0., 0.), p(1., 0.)],
            vec![0., 0., 1., 2., 2.],
        )
        .unwrap(),
    );
    assert!(
        source
            .as_ref()
            .evaluate_with_tangent_on_side(0., ParameterSide::Right)
            .is_err()
    );
    let second = line(p(3., 1.), p(3., 2.));
    let blend = try_blend_curve(
        &source,
        p(0., 0.),
        &second,
        p(3., 1.),
        CurveBlendOptions {
            continuity: [
                CurveBlendContinuity::Position,
                CurveBlendContinuity::Tangency,
            ],
            handles: [None, Some(1.)],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(blend.degree(), 2);
    assert!(
        blend.control_points()[0]
            .point()
            .distance_to(p(0., 0.))
            .unwrap()
            < 1e-12
    );
    assert!(
        blend.control_points()[2]
            .point()
            .distance_to(p(3., 1.))
            .unwrap()
            < 1e-12
    );
}

#[test]
fn quintic_matches_circular_curvature_and_a_straight_end() {
    let diagonal = 2.0_f64.sqrt();
    let arc = CircularArc3::try_from_three_points(
        p(2., 0.),
        p(diagonal, diagonal),
        p(0., 2.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let first = Curve3::Arc(arc);
    let second = line(p(-4., 3.), p(-4., 4.));
    let blend = try_blend_curve(
        &first,
        p(0., 2.),
        &second,
        p(-4., 3.),
        CurveBlendOptions {
            continuity: [
                CurveBlendContinuity::Curvature,
                CurveBlendContinuity::Curvature,
            ],
            handles: [Some(1.), Some(1.5)],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(blend.degree(), 5);
    let expected = CurveRef::Arc(&arc)
        .curvature_vector(*arc.domain().end())
        .unwrap();
    let actual = CurveRef::NurbsCurve(&blend)
        .curvature_vector(*blend.domain().start())
        .unwrap();
    let difference = Vector3::try_new(
        actual.x() - expected.x(),
        actual.y() - expected.y(),
        actual.z() - expected.z(),
    )
    .unwrap();
    assert!(difference.length().unwrap() < 1e-12);
    let end_curvature = CurveRef::NurbsCurve(&blend)
        .curvature_vector(*blend.domain().end())
        .unwrap();
    assert!(end_curvature.length().unwrap() < 1e-12);
}

#[test]
fn quartic_matches_curvature_at_a_second_curves_start() {
    let diagonal = 2.0_f64.sqrt();
    let arc = CircularArc3::try_from_three_points(
        p(2., 0.),
        p(diagonal, diagonal),
        p(0., 2.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let blend = try_blend_curve(
        &line(p(-4., -1.), p(-4., 0.)),
        p(-4., 0.),
        &Curve3::Arc(arc),
        p(2., 0.),
        CurveBlendOptions {
            continuity: [
                CurveBlendContinuity::Tangency,
                CurveBlendContinuity::Curvature,
            ],
            handles: [Some(1.), Some(1.)],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(blend.degree(), 4);
    let expected = CurveRef::Arc(&arc)
        .curvature_vector(*arc.domain().start())
        .unwrap();
    let actual = CurveRef::NurbsCurve(&blend)
        .curvature_vector(*blend.domain().end())
        .unwrap();
    let difference = Vector3::try_new(
        actual.x() - expected.x(),
        actual.y() - expected.y(),
        actual.z() - expected.z(),
    )
    .unwrap();
    assert!(difference.length().unwrap() < 1e-12);
    let tangent = CurveRef::NurbsCurve(&blend)
        .evaluate_with_tangent_on_side(*blend.domain().end(), ParameterSide::Left)
        .unwrap()
        .tangent()
        .as_vector();
    assert!(
        tangent
            .angle_to(Vector3::try_new(0., 1., 0.).unwrap())
            .unwrap()
            < 1e-12
    );
}

#[test]
fn cubic_position_to_curvature_matches_the_curved_end() {
    let diagonal = 2.0_f64.sqrt();
    let arc = CircularArc3::try_from_three_points(
        p(2., 0.),
        p(diagonal, diagonal),
        p(0., 2.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let blend = try_blend_curve(
        &line(p(-4., -1.), p(-4., 0.)),
        p(-4., 0.),
        &Curve3::Arc(arc),
        p(2., 0.),
        CurveBlendOptions {
            continuity: [
                CurveBlendContinuity::Position,
                CurveBlendContinuity::Curvature,
            ],
            handles: [None, Some(1.)],
            ..Default::default()
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(blend.degree(), 3);
    let expected = CurveRef::Arc(&arc)
        .curvature_vector(*arc.domain().start())
        .unwrap();
    let actual = CurveRef::NurbsCurve(&blend)
        .curvature_vector(*blend.domain().end())
        .unwrap();
    let difference = Vector3::try_new(
        actual.x() - expected.x(),
        actual.y() - expected.y(),
        actual.z() - expected.z(),
    )
    .unwrap();
    assert!(difference.length().unwrap() < 1e-12);
}

#[test]
fn default_line_blends_match_rhino_common_control_shape() {
    let first = line(p(0., 0.), p(1., 0.));
    let second = line(p(4., 1.), p(4., 2.));
    let chord = 10.0_f64.sqrt();
    for (continuity, degree, expected) in [
        (
            CurveBlendContinuity::Position,
            1,
            vec![p(1., 0.), p(4., 1.)],
        ),
        (
            CurveBlendContinuity::Tangency,
            3,
            vec![p(1., 0.), p(1. + chord, 0.), p(4., 1. - chord), p(4., 1.)],
        ),
        (
            CurveBlendContinuity::Curvature,
            5,
            vec![
                p(1., 0.),
                p(1. + 0.4 * chord, 0.),
                p(1. + 0.8 * chord, 0.),
                p(4., 1. - 0.8 * chord),
                p(4., 1. - 0.4 * chord),
                p(4., 1.),
            ],
        ),
    ] {
        let blend = try_blend_curve(
            &first,
            p(1., 0.),
            &second,
            p(4., 1.),
            CurveBlendOptions {
                continuity: [continuity; 2],
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        assert_eq!(blend.degree(), degree);
        assert_eq!(blend.control_points().len(), expected.len());
        for (actual, expected) in blend.control_points().iter().zip(expected) {
            assert!(actual.point().distance_to(expected).unwrap() < 1e-12);
        }
    }
}
