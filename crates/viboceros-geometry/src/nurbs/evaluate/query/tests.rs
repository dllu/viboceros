use super::*;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn bits(jet: CurveJet) -> [[u64; 3]; 3] {
    [jet.0.to_array(), jet.1.to_array(), jet.2.to_array()].map(|v| v.map(Real::to_bits))
}

fn check(query: &mut CurveQuery<'_>, stations: &[Real]) {
    let zero = Vector3::try_new(0., 0., 0.).unwrap();
    for &parameter in stations {
        for side in [ParameterSide::Left, ParameterSide::Right] {
            for order in [0, 2, 1, 0, 2] {
                let expected = match order {
                    0 => query
                        .curve
                        .evaluate_on_side(parameter, side)
                        .map(|p| (p, zero, zero)),
                    1 => query
                        .curve
                        .evaluate_with_derivative_on_side(parameter, side)
                        .map(|(p, d)| (p, d, zero)),
                    _ => query
                        .curve
                        .evaluate_with_second_derivative_on_side(parameter, side),
                };
                assert_eq!(
                    query.jet(parameter, side, order).map(bits),
                    expected.map(bits),
                    "{parameter}, {side:?}, order {order}"
                );
            }
        }
    }
}

#[test]
fn curve_query_switches_spans_and_sides_without_mutating_the_curve() {
    let curve = NurbsCurve::try_new_rational(
        2,
        (0..6)
            .map(|i| {
                WeightedPoint3::try_new(
                    p(i as Real, (i * i) as Real, -(i as Real)),
                    if i == 4 { -0.25 } else { 1. + i as Real / 8. },
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let before = curve.clone();
    let mut query = CurveQuery::new(&curve);
    check(&mut query, &[0., 0.1, 0.3]);
    assert!(matches!(query.active, Some((2, Prepared::Float(_)))));
    check(&mut query, &[0.5, 0.7, 1.]);
    assert!(matches!(query.active, Some((5, Prepared::Exact(_)))));
    check(
        &mut query,
        &[0.1, 0.5, 0.5_f64.next_down(), 0.5_f64.next_up(), 0.2],
    );
    assert!(matches!(query.active, Some((2, Prepared::Float(_)))));
    assert_eq!(curve, before);
}

fn line(points: [Point3; 2], weights: [Real; 2], end: Real) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        1,
        points
            .into_iter()
            .zip(weights)
            .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        vec![0., 0., end, end],
    )
    .unwrap()
}

#[test]
fn curve_query_preserves_exact_poles_and_validation_after_cached_jets() {
    for sign in [-1., 1.] {
        let curve = line([p(0., 0., 0.), p(1., 0., 0.)], [sign, -2. * sign], 1.5);
        let mut query = CurveQuery::new(&curve);
        for _ in 0..2 {
            check(&mut query, &[0.25, 0.5]);
            assert_eq!(
                query.evaluate(0.5),
                Err(GeometryError::ZeroWeightAtParameter)
            );
            check(
                &mut query,
                &[
                    Real::NAN,
                    Real::INFINITY,
                    -1.,
                    2.,
                    0.5_f64.next_up(),
                    0.5_f64.next_down(),
                    1.5,
                    0.,
                ],
            );
        }
    }
}

#[test]
fn curve_query_derivative_failures_do_not_poison_point_or_lower_order_queries() {
    for end in [Real::from_bits(1), 1e-200] {
        for constant in [false, true] {
            let curve = line(
                [p(3., -4., 5.), p(if constant { 3. } else { 4. }, -4., 5.)],
                [1., 2.],
                end,
            );
            let mut query = CurveQuery::new(&curve);
            check(&mut query, &[0., end * 0.25, end, 0.]);
            assert!(matches!(query.active, Some((_, Prepared::Float(_)))));
            assert!(query.evaluate(0.).is_ok());
            if !constant && end == Real::from_bits(1) {
                assert!(matches!(
                    query.evaluate_with_derivative(0.),
                    Err(GeometryError::NonFinite { .. })
                ));
            }
        }
    }
}

#[test]
fn curve_query_retains_range_recovery_and_signed_weight_frames() {
    let huge = Real::MAX;
    let tiny = Real::from_bits(1);
    for curve in [
        line(
            [p(0.8 * huge, 0., 0.), p(0.4 * huge, 0., 0.)],
            [1., -0.5],
            1e300,
        ),
        line([p(tiny, 0., 0.), p(2. * tiny, 0., 0.)], [1., 2.], 1.),
        line([p(-huge, 0., 0.), p(huge, 0., 0.)], [1., 1.], 1.),
        line([p(0., 0., 0.), p(1., 0., 0.)], [1e-300, 1e300], 1.),
    ] {
        let mut query = CurveQuery::new(&curve);
        let end = *curve.domain().end();
        check(&mut query, &[0., end * 0.25, end * 0.5, end * 0.75, end]);
    }
}
