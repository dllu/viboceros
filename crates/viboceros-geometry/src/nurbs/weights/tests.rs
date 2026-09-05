use super::*;

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn quadratic(start: Real, scale: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            (point(start, 0.0), 1.0),
            (point(start + 1.0, 1.0), 0.5),
            (point(start + 2.0, 0.0), 2.0),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w * scale).unwrap())
        .to_vec(),
        vec![start, start, start, start + 2.0, start + 2.0, start + 2.0],
    )
    .unwrap()
}

fn closed_independent_scales() -> NurbsCurve {
    let a = 2.0_f64.powi(-700);
    let b = 2.0_f64.powi(700);
    NurbsCurve::try_new_rational(
        1,
        [
            (point(0.0, 0.0), a),
            (point(2.0, 0.0), a),
            (point(2.0, 0.0), b),
            (point(1.0, 2.0), b),
            (point(1.0, 2.0), b),
            (point(0.0, 0.0), b),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0],
    )
    .unwrap()
}

#[test]
fn appended_curves_match_scales_without_forming_their_ratio() {
    for (a, b) in [(700, -700), (-700, 700), (700, 700), (-700, -700)] {
        for sign in [1.0, -1.0] {
            let first = quadratic(0.0, 2.0_f64.powi(a));
            let second = quadratic(2.0, sign * 2.0_f64.powi(b));
            let combined = first.try_append_clamped(&second).unwrap();
            assert_eq!(combined.control_points.len(), 5);
            assert_eq!(combined.knot_multiplicity(2.0).unwrap(), 2);
            for i in 0..=64 {
                let t = i as Real / 16.0;
                let source = if t <= 2.0 { &first } else { &second };
                assert!(
                    combined
                        .evaluate(t)
                        .unwrap()
                        .distance_to(source.evaluate(t).unwrap())
                        .unwrap()
                        < 2e-13
                );
            }
        }
    }
}

#[test]
fn unrepresentable_rescaling_keeps_independent_full_order_seams() {
    let tiny = 2.0_f64.powi(-700);
    let huge = 2.0_f64.powi(700);
    let first = quadratic(0.0, tiny);
    let second = NurbsCurve::try_new_rational(
        2,
        [
            (point(2.0, 0.0), huge),
            (point(3.0, 1.0), tiny),
            (point(4.0, 0.0), huge),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![2.0, 2.0, 2.0, 4.0, 4.0, 4.0],
    )
    .unwrap();
    let combined = first.try_append_clamped(&second).unwrap();
    assert_eq!(combined.control_points.len(), 6);
    assert_eq!(combined.knot_multiplicity(2.0).unwrap(), 3);
    assert_eq!(&combined.control_points[..3], &first.control_points);
    assert_eq!(&combined.control_points[3..], &second.control_points);
    for t in [0.0, 0.5, 1.5, 2.0, 2.5, 3.5, 4.0] {
        let source = if t < 2.0 { &first } else { &second };
        assert!(
            combined
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t).unwrap())
                .unwrap()
                < 2e-13
        );
    }
}

#[test]
fn closed_seams_preserve_geometry_with_extreme_independent_weight_scales() {
    let source = closed_independent_scales();
    assert!(source.is_closed().unwrap());
    for seam in [0.5, 1.5, 2.5, -2.5, 5.5] {
        let result = source.try_change_closed_seam(seam).unwrap();
        assert_eq!(result.domain(), seam..=seam + 3.0);
        for i in 0..=96 {
            let t = result.parameter_at(i as Real / 96.0).unwrap();
            let original = t.rem_euclid(3.0);
            assert!(
                result
                    .evaluate(t)
                    .unwrap()
                    .distance_to(source.evaluate(original).unwrap())
                    .unwrap()
                    < 2e-13
            );
        }
    }
}

#[test]
fn wrapped_subcurve_can_cross_an_overflowing_homogeneous_scale_ratio() {
    let source = closed_independent_scales();
    let result = source.try_subcurve(2.5, 0.5).unwrap();
    assert_eq!(result.domain(), 2.5..=3.5);
    for i in 0..=64 {
        let t = result.parameter_at(i as Real / 64.0).unwrap();
        assert!(
            result
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(t.rem_euclid(3.0)).unwrap())
                .unwrap()
                < 2e-13
        );
    }
}
