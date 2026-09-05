use super::*;
use crate::Point3;

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

#[test]
fn end_normalization_clamps_only_the_active_curve() {
    let source = NurbsCurve::try_new_rational(
        2,
        (0..5)
            .map(|i| {
                WeightedPoint3::try_new(point(i as Real, (i % 3) as Real), 1.0 + i as Real).unwrap()
            })
            .collect(),
        vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    let clamped = source.clamped_to_active_domain().unwrap();
    let controls = clamped.control_points();
    let c = (controls.last().unwrap().weight() / controls[0].weight()).sqrt();
    let normalized = source.try_normalized_end_weights().unwrap();
    assert_eq!(normalized.domain(), source.domain());
    for i in 0..=128 {
        let s = i as Real / 128.0;
        let t = s / (c * (1.0 - s) + s);
        assert!(
            normalized
                .evaluate(3.0 * s)
                .unwrap()
                .distance_to(source.evaluate(3.0 * t).unwrap())
                .unwrap()
                < 1e-13
        );
    }
}

#[test]
fn bezier_end_weight_changes_preserve_arbitrary_target_gauges_and_signs() {
    for scale in [
        1.0,
        -1.0,
        Real::from_bits(1),
        -Real::from_bits(1),
        1e280,
        -1e280,
    ] {
        for desired_scale in [1.0, -1.0, 1e-280, -1e-280, 1e280, -1e280] {
            let mut controls = [
                (point(0.0, 0.0), 2.0),
                (point(3.0, 5.0), 3.0),
                (point(8.0, 1.0), 8.0),
            ]
            .map(|(p, w)| WeightedPoint3::try_new(p, w * scale).unwrap());
            let original = controls;
            change_bezier_end_weights(&mut controls, 4.0 * desired_scale, 9.0 * desired_scale)
                .unwrap();
            for ((actual, original), expected) in controls.iter().zip(original).zip([4.0, 4.5, 9.0])
            {
                assert_eq!(actual.point(), original.point());
                assert!((actual.weight() / desired_scale - expected).abs() < 5e-13);
            }
        }
    }
}

#[test]
fn invalid_bezier_weight_change_leaves_controls_untouched() {
    let original = [
        WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
        WeightedPoint3::try_new(point(1.0, 1.0), 1e300).unwrap(),
        WeightedPoint3::try_new(point(2.0, 0.0), 1.0).unwrap(),
    ];
    for (start, end) in [(0.0, 1.0), (1.0, -1.0), (Real::NAN, 1.0), (1e300, 1e300)] {
        let mut controls = original;
        assert!(change_bezier_end_weights(&mut controls, start, end).is_err());
        assert_eq!(controls, original);
    }
}

#[test]
fn end_normalization_does_not_magnify_subnormal_mobius_factors() {
    let tiny = Real::from_bits(1);
    for (knot, weights, expected) in [
        // c*tiny = 1.5 exactly in real arithmetic, hence v=1.5/2.5.
        // Rounding 1/c to a subnormal before division incorrectly gives 0.5.
        (tiny, [tiny, tiny, 1.5], 0.6),
        (
            1.0 - Real::EPSILON,
            [1.5, tiny, tiny],
            (1.0 - Real::EPSILON) * (tiny / Real::EPSILON) / 1.5,
        ),
    ] {
        let curve = NurbsCurve::try_new_rational(
            1,
            [point(0.0, 0.0), point(1.0, 2.0), point(3.0, 0.0)]
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0.0, 0.0, knot, 1.0, 1.0],
        )
        .unwrap()
        .try_normalized_end_weights()
        .unwrap();
        let actual = curve.knots()[2];
        assert!(
            (actual / expected - 1.0).abs() < 2e-13,
            "{actual:e} != {expected:e}"
        );
    }
}

#[test]
fn end_normalization_preserves_nearly_equal_weight_geometry() {
    // Check the analytic Bezier weight and its non-affine parameter map.
    let end: Real = 1.0 + 1e-8;
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.0, 0.0), 1.0),
            (point(3e8, 5e8), 0.5),
            (point(8e8, 1e8), end),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let normalized = curve.try_normalized_end_weights().unwrap();
    let expected_weight = 0.5 / end.sqrt();
    assert!((normalized.control_points[1].weight - expected_weight).abs() < 2e-16);
    for i in 0..=64 {
        let s = i as Real / 64.0;
        let t = s / (end.sqrt() * (1.0 - s) + s);
        let error = normalized
            .evaluate(s)
            .unwrap()
            .distance_to(curve.evaluate(t).unwrap())
            .unwrap();
        assert!(error < 5e-7, "sample {i}: {error}");
    }
}

#[test]
fn end_normalization_moves_nearly_equal_multispan_knots_to_preserve_geometry() {
    let end: Real = 1.0 + 1e-8;
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.0, 0.0), 1.0),
            (point(2e8, 3e8), 0.5),
            (point(4e8, 1e8), 1.5),
            (point(6e8, 0.0), end),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let normalized = curve.try_normalized_end_weights().unwrap();
    for i in 0..=64 {
        let s = i as Real / 64.0;
        let t = s / (end.sqrt() * (1.0 - s) + s);
        let error = normalized
            .evaluate(s)
            .unwrap()
            .distance_to(curve.evaluate(t).unwrap())
            .unwrap();
        assert!(error < 5e-7, "sample {i}: {error}");
    }
    assert_ne!(normalized.knots[3], curve.knots[3]);
}

#[test]
fn end_normalization_accepts_subnormal_common_weight_scales() {
    for scale in [Real::from_bits(1), -Real::from_bits(1)] {
        for weights in [[2.0, 1.0, 2.0], [2.0, 3.0, 8.0]] {
            let curve = NurbsCurve::try_new_rational(
                2,
                [point(0.0, 0.0), point(3.0, 5.0), point(8.0, 1.0)]
                    .into_iter()
                    .zip(weights)
                    .map(|(p, w)| WeightedPoint3::try_new(p, w * scale).unwrap())
                    .collect(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
            let normalized = curve.try_normalized_end_weights().unwrap();
            assert_eq!(normalized.control_points[0].weight, 1.0);
            assert_eq!(normalized.control_points[2].weight, 1.0);
            let expected = weights[1] / (weights[0] * weights[2]).sqrt();
            assert!((normalized.control_points[1].weight - expected).abs() < 2e-15);
        }
    }
}

#[test]
fn end_normalization_is_gauge_invariant_across_degrees_and_spans() {
    for degree in 1..=6 {
        for c in [0.5_f64, 2.0, 1.0 + 1e-9] {
            let count = degree + 4;
            let mut knots = vec![2.0; degree + 1];
            knots.extend([2.5, 3.25, 5.0]);
            knots.extend(vec![6.0; degree + 1]);
            let controls: Vec<_> = (0..count)
                .map(|i| {
                    let weight = if i == 0 {
                        1.0
                    } else if i + 1 == count {
                        c.powi(degree as i32)
                    } else {
                        0.5 + (i % 3) as Real
                    };
                    WeightedPoint3::try_new(point(i as Real, (i % 3) as Real), weight).unwrap()
                })
                .collect();
            let source = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
            let baseline = source.try_normalized_end_weights().unwrap();
            for scale in [1.0, -1.0, 1e-280, -1e-280, 1e280, -1e280] {
                let scaled = NurbsCurve::try_new_rational(
                    degree,
                    source
                        .control_points
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point, p.weight * scale).unwrap())
                        .collect(),
                    source.knots.clone(),
                )
                .unwrap()
                .try_normalized_end_weights()
                .unwrap();
                assert_eq!(scaled.domain(), source.domain());
                for (actual, expected) in scaled.control_points.iter().zip(&baseline.control_points)
                {
                    assert_eq!(actual.point, expected.point);
                    assert!((actual.weight - expected.weight).abs() < 2e-13);
                }
                for (&actual, &expected) in scaled.knots.iter().zip(&baseline.knots) {
                    assert!((actual - expected).abs() < 2e-14);
                }
                for i in 0..=128 {
                    let s = i as Real / 128.0;
                    let t = s / (c * (1.0 - s) + s);
                    let error = scaled
                        .evaluate(2.0 + 4.0 * s)
                        .unwrap()
                        .distance_to(source.evaluate(2.0 + 4.0 * t).unwrap())
                        .unwrap();
                    assert!(
                        error < 2e-12,
                        "degree {degree}, c {c}, scale {scale}: {error}"
                    );
                }
            }
        }
    }
}

#[test]
fn end_normalization_supports_beziers_with_overflowing_endpoint_ratios() {
    for weights in [[1e-300, 1.0, 1e300], [1e300, 1.0, 1e-300]] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [point(0.0, 0.0), point(3.0, 5.0), point(8.0, 1.0)]
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let normalized = curve.try_normalized_end_weights().unwrap();
        assert!((normalized.control_points[1].weight - 1.0).abs() < 2e-13);
    }
    for weights in [
        [Real::from_bits(1), Real::MAX],
        [Real::MAX, Real::from_bits(1)],
    ] {
        let line = NurbsCurve::try_new_rational(
            1,
            [point(0.0, 0.0), point(2.0, 3.0)]
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap()
        .try_normalized_end_weights()
        .unwrap();
        assert_eq!(line.control_points[0].weight, 1.0);
        assert_eq!(line.control_points[1].weight, 1.0);
        assert_eq!(line.evaluate(0.5).unwrap(), point(1.0, 1.5));
    }
}

#[test]
fn end_normalization_preserves_internal_signed_weights() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.0, 0.0), -1.0),
            (point(2.0, 3.0), 0.25),
            (point(6.0, 0.0), -4.0),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let normalized = curve.try_normalized_end_weights().unwrap();
    assert!((normalized.control_points[1].weight + 0.125).abs() < 1e-16);
    for i in 0..=64 {
        let s = i as Real / 64.0;
        let t = s / (2.0 - s);
        assert!(
            normalized
                .evaluate(s)
                .unwrap()
                .distance_to(curve.evaluate(t).unwrap())
                .unwrap()
                < 1e-13
        );
    }
}

#[test]
fn end_normalization_rejects_collapsed_knots_and_unrepresentable_weights() {
    for domain in [(0.0, 1.0), (1.0, 2.0)] {
        let (a, b) = domain;
        let curve = NurbsCurve::try_new_rational(
            2,
            [
                (point(0.0, 0.0), 1.0),
                (point(2.0, 3.0), 1.0),
                (point(4.0, 1.0), 1.0),
                (point(6.0, 0.0), 1e300),
            ]
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .to_vec(),
            vec![a, a, a, (a + b) * 0.5, b, b, b],
        )
        .unwrap();
        assert!(matches!(
            curve.try_normalized_end_weights(),
            Err(GeometryError::InvalidKnotVector { .. })
        ));
    }
    for weights in [
        [1e-300, 1e300, 1e-300],
        [1e300, 1e-300, 1e300],
        [1.0, 1.0, -1.0],
    ] {
        let curve = NurbsCurve::try_new_rational(
            2,
            [point(0.0, 0.0), point(2.0, 3.0), point(6.0, 0.0)]
                .into_iter()
                .zip(weights)
                .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
                .collect(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(curve.try_normalized_end_weights().is_err());
    }
}

#[test]
fn bezier_trim_normalization_accepts_extreme_common_weight_scales() {
    for scale in [1.0, Real::from_bits(2), -Real::from_bits(2), 1e300, -1e300] {
        let source = NurbsCurve::try_new_rational(
            2,
            [
                (point(0.0, 0.0), 1.0),
                (point(1.0, 1.0), 0.5),
                (point(2.0, 0.0), 2.0),
            ]
            .map(|(p, w)| WeightedPoint3::try_new(p, w * scale).unwrap())
            .to_vec(),
            vec![0.0, 0.0, 0.0, 2.0, 2.0, 2.0],
        )
        .unwrap();
        let trimmed = source
            .try_trimmed_with_normalized_end_weights(source.domain())
            .unwrap();
        assert_eq!(trimmed.control_points[0].weight, 1.0);
        assert_eq!(trimmed.control_points[2].weight, 1.0);
        assert!((trimmed.control_points[1].weight - 0.5 / 2.0_f64.sqrt()).abs() < 2e-14);
    }
}
