use super::*;
use crate::WeightedPoint3;

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn bits(jet: SurfaceJet2) -> [[u64; 3]; 6] {
    [
        jet.point.to_array(),
        jet.derivative_u.to_array(),
        jet.derivative_v.to_array(),
        jet.derivative_uu.to_array(),
        jet.derivative_uv.to_array(),
        jet.derivative_vv.to_array(),
    ]
    .map(|v| v.map(Real::to_bits))
}

fn check(query: &mut SurfaceQuery<'_>, stations: &[[Real; 2]]) {
    for &parameters in stations {
        for order in [0, 2, 1, 0, 2] {
            assert_eq!(
                query.jet(parameters, order).map(bits),
                query
                    .surface
                    .evaluate_jet(parameters, [ParameterSide::Right; 2], false, order)
                    .map(bits),
                "{parameters:?}, order {order}"
            );
        }
    }
}

#[test]
fn query_cache_tracks_both_spans_and_right_sides_of_full_order_knots() {
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        6,
        4,
        (0..4)
            .flat_map(|j| {
                (0..6).map(move |i| {
                    let weight = if i == 4 { -0.25 } else { 1. + i as Real / 8. };
                    WeightedPoint3::try_new(
                        p(i as Real, j as Real, (i * i + j) as Real / 3.),
                        weight,
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
        vec![0., 0., 0.25, 0.25, 1., 1.],
    )
    .unwrap();
    let before = surface.clone();
    let mut query = SurfaceQuery::new(&surface);
    check(&mut query, &[[0., 0.], [0.3, 0.1]]);
    assert!(matches!(query.active, Some(([2, 1], Prepared::Float(_)))));
    check(&mut query, &[[0.5, 0.1], [0.75, 0.2]]);
    assert!(matches!(query.active, Some(([5, 1], Prepared::Exact(_)))));
    check(&mut query, &[[0.75, 0.25], [1., 1.], [0.51, 0.9]]);
    assert!(matches!(query.active, Some(([5, 3], Prepared::Exact(_)))));
    check(&mut query, &[[0.1, 0.4], [0.3, 0.1], [0.5, 0.25]]);
    assert!(matches!(query.active, Some(([5, 3], Prepared::Exact(_)))));
    assert_eq!(surface, before);
}

fn patch(weights: [Real; 2], end: Real, constant: bool) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [0., 1.]
            .into_iter()
            .flat_map(|y| {
                weights.into_iter().enumerate().map(move |(i, w)| {
                    WeightedPoint3::try_new(
                        if constant {
                            p(3., -4., 5.)
                        } else {
                            p(i as Real, y, 0.)
                        },
                        w,
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0., 0., end, end],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn query_cache_preserves_poles_and_validation_without_poisoning_later_stations() {
    for sign in [-1., 1.] {
        let surface = patch([sign, -2. * sign], 1.5, false);
        let mut query = SurfaceQuery::new(&surface);
        for _ in 0..2 {
            check(&mut query, &[[0.25, 0.2], [0.5, 0.2]]);
            assert_eq!(
                query.evaluate(0.5, 0.2),
                Err(GeometryError::ZeroWeightAtParameter)
            );
            check(
                &mut query,
                &[
                    [Real::NAN, Real::INFINITY],
                    [-1., Real::NAN],
                    [0.5, Real::NAN],
                    [0.5, 2.],
                    [Real::INFINITY, 0.2],
                    [0.5_f64.next_up(), 0.2],
                    [0.5_f64.next_down(), 0.2],
                    [1.5, 1.],
                    [0., 0.],
                ],
            );
        }
    }
}

#[test]
fn query_cache_retains_float_dispatch_after_late_exact_recovery_or_overflow() {
    let tiny = Real::from_bits(1);
    for constant in [true, false] {
        let surface = patch([1., 2.], tiny, constant);
        let mut query = SurfaceQuery::new(&surface);
        check(&mut query, &[[0., 0.25], [tiny, 1.], [0., 0.]]);
        // Float point evaluation is still the public policy even when a
        // derivative request required exact recovery or failed to round.
        assert!(matches!(query.active, Some((_, Prepared::Float(_)))));
        let second = query.evaluate_with_second_derivatives(0., 0.25);
        if constant {
            assert_eq!(second.unwrap().derivative_u.to_array(), [0.; 3]);
        } else {
            assert!(matches!(second, Err(GeometryError::NonFinite { .. })));
        }
        assert!(query.evaluate(0., 0.25).is_ok());
    }
}
