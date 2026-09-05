use super::*;
use crate::ParameterSide::{Left, Right};
use crate::morph::{curve_fit::rational as candidate, denominator};

struct Cubic;
impl PointMorph for Cubic {
    fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
        Point3::try_new(
            p.x(),
            p.y(),
            p.z() + p.x().powi(2) + 0.25 * p.x() * p.y() + p.y().powi(3),
        )
    }
}

fn weighted_curve(degree: usize, weights: &[Real]) -> NurbsCurve {
    let controls = weights
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let t = i as Real / degree as Real;
            WeightedPoint3::try_new(point(t - 0.5, 0.1 + t, 0.2 * t * t), w).unwrap()
        })
        .collect();
    NurbsCurve::try_new_rational(
        degree,
        controls,
        std::iter::repeat_n(-7.0, degree + 1)
            .chain(std::iter::repeat_n(13.0, degree + 1))
            .collect(),
    )
    .unwrap()
}

fn check_image(source: &NurbsCurve, fitted: &NurbsCurve, morph: &impl PointMorph, epsilon: Real) {
    assert_eq!(source.domain(), fitted.domain());
    for (start, end) in source.spans() {
        for i in 0..=128 {
            let fraction = match i {
                0 => 0.0,
                128 => 1.0,
                _ => (i as Real - 0.3819660112501051) / 128.0,
            };
            let t = stable_lerp(start, end, fraction).unwrap();
            for side in [Left, Right] {
                let expected = morph
                    .morph_point(source.evaluate_on_side(t, side).unwrap())
                    .unwrap();
                let error = fitted
                    .evaluate_on_side(t, side)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap();
                assert!(error <= epsilon, "error {error} at {t} on {side:?}");
            }
        }
    }
}

#[test]
fn composition_preserves_cubed_denominator_under_common_sign_and_scale_changes() {
    for degree in 1..=3 {
        for scale in [1.0, -1.0, 1e-200, -1e200] {
            let weights = (0..=degree)
                .map(|i| (1.0 + i as Real) * scale)
                .collect::<Vec<_>>();
            let source = weighted_curve(degree, &weights);
            let fitted = Cubic
                .morph_nurbs_curve(&source, Tolerance::DEFAULT)
                .unwrap();
            assert_eq!(fitted.degree(), 3 * degree);
            check_image(&source, &fitted, &Cubic, 2e-12);
            let expected = denominator::curve_weights(&source).unwrap();
            let actual = NurbsCurve::try_new(
                fitted.degree(),
                fitted
                    .control_points()
                    .iter()
                    .map(|c| point(c.weight(), 0.0, 0.0))
                    .collect(),
                fitted.knots().to_vec(),
            )
            .unwrap();
            for i in 0..=128 {
                let t = source.parameter_at(i as Real / 128.0).unwrap();
                let expected = expected.evaluate(t).unwrap().x().powi(3);
                assert!((actual.evaluate(t).unwrap().x() - expected).abs() < 2e-13);
            }
        }
    }
}

#[test]
fn composition_keeps_both_full_order_limits_and_their_derivatives() {
    let source = NurbsCurve::try_new_rational(
        2,
        (0..6)
            .map(|i| {
                WeightedPoint3::try_new(
                    point(i as Real * 0.2, (i as Real * 0.4).sin(), 0.0),
                    1.0 + i as Real * 0.1,
                )
                .unwrap()
            })
            .collect(),
        vec![-7.0, -7.0, -7.0, -2.0, -2.0, -2.0, 13.0, 13.0, 13.0],
    )
    .unwrap();
    let fitted = Cubic
        .morph_nurbs_curve(&source, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(fitted.knots().iter().filter(|&&k| k == -2.0).count(), 7);
    check_image(&source, &fitted, &Cubic, 2e-12);
    for t in [-7.0, -2.0, 13.0] {
        for side in [Left, Right] {
            let (p, d) = source.evaluate_with_derivative_on_side(t, side).unwrap();
            let (_, actual) = fitted.evaluate_with_derivative_on_side(t, side).unwrap();
            let expected = [
                d.x(),
                d.y(),
                d.z()
                    + (2.0 * p.x() + 0.25 * p.y()) * d.x()
                    + (0.25 * p.x() + 3.0 * p.y().powi(2)) * d.y(),
            ];
            for (a, b) in actual.to_array().into_iter().zip(expected) {
                assert!((a - b).abs() < 2e-12, "derivative {a} != {b}");
            }
        }
    }
}

#[test]
fn affine_map_retains_the_original_rational_control_net() {
    struct Affine;
    impl PointMorph for Affine {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x() + 2.0 * p.y() + 3.0, p.y() - p.z(), 4.0 * p.z())
        }
    }
    // Even a mixed-sign net with no pole need not lose its exact affine image.
    for weights in [[1.0, 0.3, 2.0], [1.0, -0.1, 2.0]] {
        let source = weighted_curve(2, &weights);
        let fitted = Affine
            .morph_nurbs_curve(&source, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(fitted.degree(), source.degree());
        assert_eq!(fitted.knots(), source.knots());
        for (a, b) in source.control_points().iter().zip(fitted.control_points()) {
            assert_eq!(a.weight(), b.weight());
            assert_eq!(Affine.morph_point(a.point()).unwrap(), b.point());
        }
        check_image(&source, &fitted, &Affine, 2e-12);
    }
}

#[test]
fn a_noncubic_map_rejects_the_candidate_and_refines_to_tolerance() {
    struct Wave;
    impl PointMorph for Wave {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            Point3::try_new(p.x(), p.y(), (8.0 * p.x()).sin())
        }
    }
    let source = weighted_curve(2, &[1.0, 2.0, 3.0]);
    let unfitted = candidate::candidate(
        &mut |t, side| Wave.morph_point(source.evaluate_on_side(t, side)?),
        &source,
        512,
    )
    .unwrap()
    .unwrap();
    let exact = Wave.morph_point(source.evaluate(-1.3).unwrap()).unwrap();
    assert!(unfitted.evaluate(-1.3).unwrap().distance_to(exact).unwrap() > 1e-4);
    let fitted = Wave
        .morph_nurbs_curve(&source, Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap())
        .unwrap();
    assert_eq!(fitted.degree(), 3);
    assert!(!fitted.is_rational());
    check_image(&source, &fitted, &Wave, 1e-7);
}

#[test]
fn off_curve_control_map_errors_do_not_reject_a_valid_image() {
    struct OnCircle;
    impl PointMorph for OnCircle {
        fn morph_point(&self, p: Point3) -> Result<Point3, GeometryError> {
            if (p.x().powi(2) + p.y().powi(2) - 0.16).abs() > 1e-12 {
                return Err(GeometryError::Degenerate {
                    context: "outside circle",
                });
            }
            Cubic.morph_point(p)
        }
    }
    let source = crate::Circle3::try_new(
        point(0.0, 0.0, 0.0),
        0.4,
        crate::UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    assert!(candidate::mapped_controls(&OnCircle, &source).is_err());
    let fitted = OnCircle
        .morph_nurbs_curve(&source, Tolerance::DEFAULT)
        .unwrap();
    check_image(&source, &fitted, &OnCircle, 2e-12);
}

#[test]
fn invalid_or_oversized_candidate_spaces_do_not_sample_the_map() {
    for source in [
        weighted_curve(2, &[1.0, 1.0, 1.0]),
        weighted_curve(2, &[1.0, -0.1, 2.0]),
        weighted_curve(2, &[Real::from_bits(1), 1e300, 1.0]),
        weighted_curve(2, &[1e-200, 1.0, 2.0]), // W is representable but W³ underflows.
        weighted_curve(4, &[1.0, 2.0, 3.0, 4.0, 5.0]),
    ] {
        assert!(
            candidate::candidate(&mut |_, _| panic!("unsupported space"), &source, 512)
                .unwrap()
                .is_none()
        );
    }
    assert!(
        candidate::candidate(
            &mut |_, _| panic!("over control budget"),
            &weighted_curve(2, &[1.0, 2.0, 3.0]),
            6
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn candidate_source_mapping_errors_propagate_without_retry() {
    let mut calls = 0;
    let result = candidate::candidate(
        &mut |_, _| {
            calls += 1;
            Err(GeometryError::Degenerate {
                context: "test map failure",
            })
        },
        &weighted_curve(2, &[1.0, 2.0, 3.0]),
        512,
    );
    assert!(matches!(
        result,
        Err(GeometryError::Degenerate {
            context: "test map failure"
        })
    ));
    assert_eq!(calls, 1);
}

#[test]
fn rational_interpolation_keeps_constant_large_coordinate_targets_exact() {
    let expected = point(1e12, -2e12, 3e12);
    let fitted = candidate::candidate(
        &mut |_, _| Ok(expected),
        &weighted_curve(3, &[1.0, 2.0, 3.0, 4.0]),
        512,
    )
    .unwrap()
    .unwrap();
    assert!(
        fitted
            .control_points()
            .iter()
            .all(|c| c.point() == expected)
    );
}

#[test]
fn an_unclamped_rational_source_retains_its_active_domain_and_seam() {
    let source = NurbsCurve::try_new_rational(
        2,
        [
            (0.4, 0.0),
            (0.0, 0.4),
            (-0.4, 0.0),
            (0.0, -0.4),
            (0.4, 0.0),
            (0.0, 0.4),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| {
            WeightedPoint3::try_new(point(x, y, 0.0), if i % 2 == 0 { 1.0 } else { 0.7 }).unwrap()
        })
        .collect(),
        (-2..=6).map(Real::from).collect(),
    )
    .unwrap();
    let fitted = Cubic
        .morph_nurbs_curve(&source, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(fitted.degree(), 6);
    check_image(&source, &fitted, &Cubic, 2e-12);
    assert!(
        fitted
            .evaluate(*fitted.domain().start())
            .unwrap()
            .distance_to(fitted.evaluate(*fitted.domain().end()).unwrap())
            .unwrap()
            < 2e-12
    );
}
