use super::*;
use crate::ParameterSide::{Left, Right};
use crate::{NurbsCurve, Point3, WeightedPoint3};

fn p(x: Real, y: Real) -> Point2 {
    Point2::try_new(x, y).unwrap()
}

fn lifted(curve: &NurbsCurve2) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        curve.degree,
        curve
            .control_points
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_new(c.point.x(), c.point.y(), 0.0).unwrap(),
                    c.weight,
                )
                .unwrap()
            })
            .collect(),
        curve.knots.clone(),
    )
    .unwrap()
}

#[test]
fn constant_uv_coordinates_and_their_derivatives_remain_exact() {
    for value in [0.4, -0.4, 1e12] {
        for scale in [1.0, -1.0, 1e-200, -1e200] {
            let curve = NurbsCurve2::try_new_rational(
                2,
                [
                    (p(0.0, value), 1.0),
                    (p(2.0, value), 0.7),
                    (p(5.0, value), 1.2),
                ]
                .map(|(p, w)| WeightedPoint2::try_new(p, w * scale).unwrap())
                .to_vec(),
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
            for i in 0..=1000 {
                let t = i as Real / 1000.0;
                assert_eq!(curve.evaluate(t).unwrap().y(), value);
                let (point, derivative) = curve.evaluate_with_derivative(t).unwrap();
                assert_eq!(point.y(), value);
                assert_eq!(derivative[1], 0.0);
            }
        }
    }
}

#[test]
fn sided_uv_points_and_tangents_agree_with_the_lifted_curve() {
    let curve = NurbsCurve2::try_new_rational(
        1,
        [
            (p(0.0, 0.4), 1.0),
            (p(1.0, 0.4), 2.0),
            (p(3.0, 0.7), 3.0),
            (p(4.0, 0.7), 4.0),
        ]
        .map(|(p, w)| WeightedPoint2::try_new(p, w).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    let reference = lifted(&curve);
    for t in [0.0, 0.381, 1.0, 1.618, 2.0] {
        for side in [Left, Right] {
            let (p, d) = curve.evaluate_with_derivative_on_side(t, side).unwrap();
            let (q, e) = reference.evaluate_with_derivative_on_side(t, side).unwrap();
            assert_eq!(curve.evaluate_on_side(t, side).unwrap(), p);
            assert_eq!(p.to_array(), [q.x(), q.y()]);
            assert_eq!(d, [e.x(), e.y()]);
        }
    }
    assert_eq!(curve.evaluate_on_side(1.0, Left).unwrap(), p(1.0, 0.4));
    assert_eq!(curve.evaluate_on_side(1.0, Right).unwrap(), p(3.0, 0.7));
}

#[test]
fn translated_rational_uv_jets_do_not_cancel_large_world_offsets() {
    let source = NurbsCurve2::try_new_rational(
        2,
        [(p(0.0, 0.0), 1.0), (p(2.0, 3.0), -0.2), (p(5.0, 1.0), 1.0)]
            .map(|(p, w)| WeightedPoint2::try_new(p, w).unwrap())
            .to_vec(),
        vec![-2.0, -2.0, -2.0, 3.0, 3.0, 3.0],
    )
    .unwrap();
    let translated = NurbsCurve2::try_new_rational(
        2,
        source
            .control_points
            .iter()
            .map(|c| {
                WeightedPoint2::try_new(p(c.point.x() + 1e12, c.point.y() - 2e12), c.weight)
                    .unwrap()
            })
            .collect(),
        source.knots.clone(),
    )
    .unwrap();
    for i in 0..=32 {
        let t = source.parameter_at(i as Real / 32.0).unwrap();
        let (_, expected) = source.evaluate_with_derivative(t).unwrap();
        let (_, actual) = translated.evaluate_with_derivative(t).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn signed_uv_images_retry_unshifted_without_hiding_real_poles() {
    let bound = Real::MAX;
    let curve = NurbsCurve2::try_new_rational(
        1,
        [(p(0.8 * bound, 0.4), 1.0), (p(0.4 * bound, 0.4), -0.5)]
            .map(|(p, w)| WeightedPoint2::try_new(p, w).unwrap())
            .to_vec(),
        vec![0.0, 0.0, 1e300, 1e300],
    )
    .unwrap();
    let point = curve.evaluate(0.75e300).unwrap();
    assert!((point.x() / bound + 0.4).abs() < 2e-15);
    assert_eq!(point.y(), 0.4);
    let (point, derivative) = curve.evaluate_with_derivative(0.75e300).unwrap();
    assert!((point.x() / bound + 0.4).abs() < 2e-15);
    assert!((derivative[0] / (bound / 1e300) - 12.8).abs() < 2e-13);
    assert_eq!(point.y(), 0.4);
    assert_eq!(derivative[1], 0.0);
    let pole = NurbsCurve2::try_new_rational(
        1,
        [(p(1e12, 0.4), 1.0), (p(1e12 + 4.0, 0.4), -1.0)]
            .map(|(p, w)| WeightedPoint2::try_new(p, w).unwrap())
            .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(
        pole.evaluate(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
    assert_eq!(
        pole.evaluate_with_derivative(0.5),
        Err(GeometryError::ZeroWeightAtParameter)
    );
}
