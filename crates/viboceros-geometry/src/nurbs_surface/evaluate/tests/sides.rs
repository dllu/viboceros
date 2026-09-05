use super::*;
use crate::ParameterSide::{Left, Right};

#[test]
fn four_exact_limits_at_crossed_full_order_knot_lines_remain_distinct() {
    let xs = [0.0, 1.0, 5.0, 7.0];
    let ys = [0.0, 3.0, 10.0, 14.0];
    let source = NurbsSurface::try_new_rational(
        1,
        1,
        4,
        4,
        ys.into_iter()
            .flat_map(|y| {
                xs.into_iter()
                    .map(move |x| WeightedPoint3::try_new(p(x, y, x * y), 1.0).unwrap())
            })
            .collect(),
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    )
    .unwrap();
    for (su, x, dx) in [(Left, 1.0, 1.0), (Right, 5.0, 2.0)] {
        for (sv, y, dy) in [(Left, 3.0, 3.0), (Right, 10.0, 4.0)] {
            let jet = source
                .evaluate_with_second_derivatives_on_sides(1.0, 1.0, su, sv)
                .unwrap();
            assert_eq!(jet.point, p(x, y, x * y));
            near(jet.derivative_u, [dx, 0.0, dx * y]);
            near(jet.derivative_v, [0.0, dy, x * dy]);
            near(jet.derivative_uu, [0.0; 3]);
            near(jet.derivative_uv, [0.0, 0.0, dx * dy]);
            near(jet.derivative_vv, [0.0; 3]);
        }
    }
    for t in [0.0, 2.0] {
        let expected = source.evaluate_with_second_derivatives(t, t).unwrap();
        for su in [Left, Right] {
            for sv in [Left, Right] {
                assert_eq!(
                    source
                        .evaluate_with_second_derivatives_on_sides(t, t, su, sv)
                        .unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn shared_degree_multiple_control_keeps_distinct_partial_limits() {
    let row = [
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(2.0, 0.0, 0.0),
        p(2.0, 2.0, 0.0),
        p(4.0, 2.0, 0.0),
    ];
    let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0];
    let reference = NurbsCurve::try_new(2, row.to_vec(), knots.clone()).unwrap();
    let source = NurbsSurface::try_new_rational(
        2,
        1,
        5,
        2,
        [0.0, 1.0]
            .into_iter()
            .flat_map(|z| {
                row.map(move |p| {
                    WeightedPoint3::try_new(
                        p.translated(Vector3::try_new(0.0, 0.0, z).unwrap())
                            .unwrap(),
                        1.0,
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots,
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    for side in [Left, Right] {
        let expected = reference
            .evaluate_with_second_derivative_on_side(1.0, side)
            .unwrap();
        let actual = source
            .evaluate_with_second_derivatives_on_sides(1.0, 0.3, side, Right)
            .unwrap();
        assert_eq!(actual.point, p(2.0, 0.0, 0.3));
        near(actual.derivative_u, expected.1.to_array());
        near(actual.derivative_v, [0.0, 0.0, 1.0]);
        near(actual.derivative_uu, expected.2.to_array());
        near(actual.derivative_uv, [0.0; 3]);
        near(actual.derivative_vv, [0.0; 3]);
    }
}

#[test]
fn periodic_unclamped_surface_boundaries_agree_with_native_control_curves() {
    let row = [
        p(0.0, 0.0, 0.0),
        p(3.0, 0.0, 0.0),
        p(0.0, 3.0, 0.0),
        p(0.0, 0.0, 0.0),
        p(3.0, 0.0, 0.0),
    ];
    let knots = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let reference = NurbsCurve::try_new(2, row.to_vec(), knots.clone()).unwrap();
    let source = NurbsSurface::try_new_rational(
        2,
        1,
        5,
        2,
        [0.0, 2.0]
            .into_iter()
            .flat_map(|z| {
                row.map(move |p| {
                    WeightedPoint3::try_new(
                        p.translated(Vector3::try_new(0.0, 0.0, z).unwrap())
                            .unwrap(),
                        1.0,
                    )
                    .unwrap()
                })
            })
            .collect(),
        knots,
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    for u in [0.0, 0.4, 1.0, 2.3, 3.0] {
        let expected = reference.evaluate_with_second_derivative(u).unwrap();
        let actual = source.evaluate_with_second_derivatives(u, 0.3).unwrap();
        assert!(
            actual
                .point
                .distance_to(
                    expected
                        .0
                        .translated(Vector3::try_new(0.0, 0.0, 0.6).unwrap())
                        .unwrap()
                )
                .unwrap()
                < 2e-12
        );
        near(actual.derivative_u, expected.1.to_array());
        near(actual.derivative_v, [0.0, 0.0, 2.0]);
        near(actual.derivative_uu, expected.2.to_array());
        near(actual.derivative_uv, [0.0; 3]);
        near(actual.derivative_vv, [0.0; 3]);
    }
}
