use super::*;

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn rational_line(offset: Real, factor: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(point(offset, -offset, offset), factor).unwrap(),
            WeightedPoint3::try_new(point(offset + 4.0, -offset, offset), factor * 2.0).unwrap(),
        ],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap()
}

#[test]
fn unequal_degree_one_weights_have_nonzero_second_derivatives() {
    for offset in [0.0, 1e12] {
        for scale in [1.0, 2.0_f64.powi(-600), 2.0_f64.powi(600), -1.0] {
            let curve = rational_line(offset, scale);
            for i in 0..=32 {
                let t = i as Real / 32.0;
                let (p, d, dd) = curve.evaluate_with_second_derivative(t).unwrap();
                assert_eq!(p, point(offset + 8.0 * t / (1.0 + t), -offset, offset));
                assert!((d.x() - 8.0 / (1.0 + t).powi(2)).abs() < 2e-13);
                assert!((dd.x() + 16.0 / (1.0 + t).powi(3)).abs() < 2e-13);
                assert_eq!([d.y(), d.z(), dd.y(), dd.z()], [0.0; 4]);
            }
        }
    }
}

#[test]
fn rational_jets_are_translation_invariant_in_local_control_coordinates() {
    let base = NurbsCurve::try_new_rational(
        3,
        [
            (point(0.0, 0.0, 0.0), 1.0),
            (point(2.0, 3.0, 1.0), 0.25),
            (point(5.0, -2.0, 4.0), 2.0),
            (point(7.0, 1.0, -1.0), 0.5),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![-7.0, -7.0, -7.0, -7.0, 13.0, 13.0, 13.0, 13.0],
    )
    .unwrap();
    let translated = base
        .transformed(AffineTransform3::from_translation(
            Vector3::try_new(1e12, -2e12, 3e12).unwrap(),
        ))
        .unwrap();
    for i in 0..=64 {
        let t = base.parameter_at(i as Real / 64.0).unwrap();
        let (_, d, dd) = base.evaluate_with_second_derivative(t).unwrap();
        let (_, td, tdd) = translated.evaluate_with_second_derivative(t).unwrap();
        assert_eq!(d, td);
        assert_eq!(dd, tdd);
    }
}

#[test]
fn constant_rational_curves_have_zero_jets_even_far_from_the_origin() {
    let p = point(1e200, -2e200, 3e200);
    let curve = NurbsCurve::try_new_rational(
        1,
        [1.0, 3.0]
            .map(|w| WeightedPoint3::try_new(p, w).unwrap())
            .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let (actual, d, dd) = curve.evaluate_with_second_derivative(0.25).unwrap();
    assert_eq!(actual, p);
    assert_eq!(d.to_array(), [0.0; 3]);
    assert_eq!(dd.to_array(), [0.0; 3]);
}

#[test]
fn signed_weight_points_can_remain_finite_when_the_local_offset_overflows() {
    let bound = Real::MAX;
    let curve = NurbsCurve::try_new_rational(
        1,
        [
            (point(0.8 * bound, 0.0, 0.0), 1.0),
            (point(0.4 * bound, 0.0, 0.0), -0.5),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1e300, 1e300],
    )
    .unwrap();
    let t = 0.75e300;
    assert!((curve.evaluate(t).unwrap().x() / bound + 0.4).abs() < 2e-15);
    let (p, d, dd) = curve.evaluate_with_second_derivative(t).unwrap();
    assert!((p.x() / bound + 0.4).abs() < 2e-15);
    assert!((d.x() / (bound / 1e300) - 12.8).abs() < 2e-13);
    assert!((dd.x() / (bound / 1e300 / 1e300) + 307.2).abs() < 2e-11);
}

#[test]
fn genuine_rational_poles_are_not_hidden_by_frame_fallback() {
    let curve = NurbsCurve::try_new_rational(
        1,
        [
            (point(1e12, 0.0, 0.0), 1.0),
            (point(1e12 + 4.0, 0.0, 0.0), -1.0),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(
        curve.evaluate(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(
        curve.evaluate_with_derivative(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(
        curve.evaluate_with_second_derivative(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
}

#[test]
fn interpolated_span_endpoints_retain_the_exact_control_point() {
    let curve = NurbsCurve::try_new_rational(
        2,
        [
            (point(0.1, 0.8, 0.0), 1e-200),
            (point(0.3, 0.6, 0.0), 1e200),
            (point(0.7, 0.1, 0.0), 1e-200),
            (point(0.7, 0.1, 0.0), 2.0),
            (point(0.8, 0.4, 0.0), 0.5),
            (point(0.9, 0.6, 0.0), 1.0),
        ]
        .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    assert_eq!(curve.evaluate(0.0).unwrap(), curve.control_points[0].point);
    assert_eq!(curve.evaluate(1.0).unwrap(), curve.control_points[3].point);
    assert_eq!(curve.evaluate(2.0).unwrap(), curve.control_points[5].point);
    let (p, _, _) = curve.evaluate_with_second_derivative(2.0).unwrap();
    assert_eq!(p, curve.control_points[5].point);
}
