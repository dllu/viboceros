use super::tests::{ellipse, near, near_vector, p};
use super::*;
use crate::{CurveSegment3, NurbsCurve, PolyCurve3, Polyline3, Tolerance, WeightedPoint3};

fn rational(degree: usize, controls: &[(Point3, Real)], knots: Vec<Real>) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        degree,
        controls
            .iter()
            .map(|&(p, w)| WeightedPoint3::try_new(p, w).unwrap())
            .collect(),
        knots,
    )
    .unwrap()
}

#[test]
fn polynomial_and_rational_knots_supply_exact_one_sided_jets() {
    let curve = rational(
        2,
        &[
            (p(0.0, 0.0, 0.0), 1.0),
            (p(1.0, 0.0, 0.0), 2.0),
            (p(2.0, 0.0, 0.0), 1.0),
            (p(2.0, 2.0, 0.0), 0.5),
            (p(3.0, 3.0, 0.0), 3.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    let (left, right) = curve.try_split(1.0).unwrap();
    for (side, piece, first, second) in [
        (
            CurveEvaluationSide::Left,
            left,
            [4.0, 0.0, 0.0],
            [20.0, 0.0, 0.0],
        ),
        (
            CurveEvaluationSide::Right,
            right,
            [0.0, 2.0, 0.0],
            [6.0, 18.0, 0.0],
        ),
    ] {
        let jet = curve
            .evaluate_with_second_derivative_on_side(1.0, side)
            .unwrap();
        assert_eq!(jet.0, p(2.0, 0.0, 0.0));
        near_vector(jet.1, Vector3::try_from(first).unwrap());
        near_vector(jet.2, Vector3::try_from(second).unwrap());
        let split_jet = piece.evaluate_with_second_derivative(1.0).unwrap();
        near_vector(jet.1, split_jet.1);
        near_vector(jet.2, split_jet.2);
    }
}

#[test]
fn full_order_knots_preserve_distinct_point_limits() {
    let curve = rational(
        1,
        &[
            (p(0.0, 0.0, 0.0), 1.0),
            (p(1.0, 0.0, 0.0), 2.0),
            (p(10.0, 2.0, 0.0), 0.5),
            (p(10.0, 6.0, 0.0), 1.0),
        ],
        vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0],
    );
    let view = CurveRef::NurbsCurve(&curve);
    for (side, point, first, second) in [
        (
            CurveEvaluationSide::Left,
            p(1.0, 0.0, 0.0),
            [0.5, 0.0, 0.0],
            [-0.5, 0.0, 0.0],
        ),
        (
            CurveEvaluationSide::Right,
            p(10.0, 2.0, 0.0),
            [0.0, 8.0, 0.0],
            [0.0, -16.0, 0.0],
        ),
    ] {
        assert_eq!(view.evaluate_on_side(1.0, side).unwrap(), point);
        let jet = view
            .evaluate_with_second_derivative_on_side(1.0, side)
            .unwrap();
        assert_eq!(jet.0, point);
        assert_eq!(jet.1.to_array(), first);
        assert_eq!(jet.2.to_array(), second);
        let first_only = view.evaluate_with_derivative_on_side(1.0, side).unwrap();
        assert_eq!(first_only, (jet.0, jet.1));
        assert_eq!(
            view.evaluate_with_tangent_on_side(1.0, side)
                .unwrap()
                .point(),
            point
        );
    }
}

#[test]
fn polycurve_side_selection_reaches_knots_inside_each_leaf_type() {
    let polyline = Polyline3::try_new(
        vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 2.0, 0.0)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for leaf in [
        CurveSegment3::Polyline(polyline.clone()),
        CurveSegment3::NurbsCurve(polyline.to_native_nurbs().unwrap()),
    ] {
        let curve = PolyCurve3::try_with_segment_domains(vec![leaf], vec![-7.0, 13.0]).unwrap();
        for (side, expected) in [
            (CurveEvaluationSide::Left, [0.1, 0.0, 0.0]),
            (CurveEvaluationSide::Right, [0.0, 0.2, 0.0]),
        ] {
            let jet = curve.evaluate_with_second_derivative(3.0, side).unwrap();
            assert_eq!(jet.0, p(1.0, 0.0, 0.0));
            assert_eq!(jet.1.to_array(), expected);
            assert_eq!(jet.2.to_array(), [0.0; 3]);
            let sample = CurveRef::PolyCurve(&curve)
                .evaluate_with_tangent_on_side(3.0, side)
                .unwrap();
            assert_eq!(sample.tangent(), jet.1.normalized_nonzero().unwrap());
        }
    }
}

#[test]
fn one_sided_limits_do_not_replace_the_parameter_with_a_neighboring_float() {
    let start: Real = 1e16;
    let knot = start + 2.0;
    let end = start + 4.0;
    assert_eq!(knot.next_down(), start);
    let curve = NurbsCurve::try_new(
        2,
        vec![
            p(0.0, 0.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(1.0, 2.0, 0.0),
            p(2.0, 2.0, 0.0),
        ],
        vec![start, start, start, knot, knot, end, end, end],
    )
    .unwrap();
    let left = curve
        .evaluate_with_derivative_on_side(knot, CurveEvaluationSide::Left)
        .unwrap();
    assert_eq!(left.0, p(1.0, 1.0, 0.0));
    assert_eq!(left.1.to_array(), [1.0, 0.0, 0.0]);
    assert_eq!(
        curve.derivative_at(knot.next_down()).unwrap().to_array(),
        [0.0, 1.0, 0.0]
    );
}

#[test]
fn ellipse_quarter_knots_use_the_requested_second_derivative() {
    for curve in [
        ellipse(),
        ellipse().try_reparameterized(-7.0..=13.0).unwrap(),
        ellipse().reversed(),
    ] {
        let rational = curve.to_nurbs().unwrap();
        let view = CurveRef::Ellipse(&curve);
        for i in 0..=4 {
            let t = view.parameter_at(i as Real / 4.0).unwrap();
            for side in [CurveEvaluationSide::Left, CurveEvaluationSide::Right] {
                let actual = view
                    .evaluate_with_second_derivative_on_side(t, side)
                    .unwrap();
                let expected = rational
                    .evaluate_with_second_derivative_on_side(t, side)
                    .unwrap();
                near(actual.0, expected.0);
                near_vector(actual.1, expected.1);
                near_vector(actual.2, expected.2);
            }
        }
    }
}

#[test]
fn reversal_swaps_sides_and_changes_only_odd_derivative_signs() {
    let curve = rational(
        2,
        &[
            (p(0.0, 0.0, 0.0), 1.0),
            (p(1.0, 0.0, 0.0), 2.0),
            (p(2.0, 0.0, 0.0), 1.0),
            (p(2.0, 2.0, 0.0), 0.5),
            (p(3.0, 3.0, 0.0), 3.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    let reversed = curve.reversed().unwrap();
    for (side, opposite) in [
        (CurveEvaluationSide::Left, CurveEvaluationSide::Right),
        (CurveEvaluationSide::Right, CurveEvaluationSide::Left),
    ] {
        let a = curve
            .evaluate_with_second_derivative_on_side(1.0, side)
            .unwrap();
        let b = reversed
            .evaluate_with_second_derivative_on_side(-1.0, opposite)
            .unwrap();
        near(a.0, b.0);
        near_vector(a.1, b.1.scaled(-1.0).unwrap());
        near_vector(a.2, b.2);
    }
}

#[test]
fn arc_length_kinks_resolve_angles_smaller_than_cosine_roundoff() {
    let curve = Polyline3::try_new(
        vec![p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(2.0, 1e-8, 0.0)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sampler =
        crate::curve::ArcLengthSampler::try_new(CurveRef::Polyline(&curve), Tolerance::DEFAULT)
            .unwrap();
    let kinks = sampler.kinks(1e-10).unwrap();
    assert_eq!(kinks.len(), 1);
    assert_eq!(kinks[0].distance, 1.0);
    assert!(sampler.kinks(1e-7).unwrap().is_empty());
}

#[test]
fn sides_at_open_and_closed_domain_endpoints_use_the_interior_span() {
    for curve in [
        ellipse().to_nurbs().unwrap(),
        rational(
            1,
            &[
                (p(0.0, 0.0, 0.0), 1.0),
                (p(1.0, 0.0, 0.0), 2.0),
                (p(1.0, 2.0, 0.0), 3.0),
            ],
            vec![0.0, 0.0, 1.0, 2.0, 2.0],
        ),
    ] {
        for t in [*curve.domain().start(), *curve.domain().end()] {
            assert_eq!(
                curve
                    .evaluate_with_second_derivative_on_side(t, CurveEvaluationSide::Left)
                    .unwrap(),
                curve
                    .evaluate_with_second_derivative_on_side(t, CurveEvaluationSide::Right)
                    .unwrap()
            );
        }
        for t in [
            Real::NAN,
            Real::INFINITY,
            *curve.domain().start() - 1.0,
            *curve.domain().end() + 1.0,
        ] {
            for side in [CurveEvaluationSide::Left, CurveEvaluationSide::Right] {
                assert!(curve.evaluate_on_side(t, side).is_err());
                assert!(curve.evaluate_with_derivative_on_side(t, side).is_err());
                assert!(
                    curve
                        .evaluate_with_second_derivative_on_side(t, side)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn stationary_points_distinguish_smooth_stalls_from_cusps() {
    // C(t)=(t-1)^3 and C(t)=(t-1)^2 with an inserted knot at t=1.
    let stall = NurbsCurve::try_new(
        3,
        vec![
            p(-1.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    let cusp = NurbsCurve::try_new(
        2,
        vec![
            p(1.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
    )
    .unwrap();
    for (curve, left_x, kink_count) in [(&stall, 1.0, 0), (&cusp, -1.0, 1)] {
        assert_eq!(curve.derivative_at(1.0).unwrap().to_array(), [0.0; 3]);
        let view = CurveRef::NurbsCurve(curve);
        assert_eq!(
            view.evaluate_with_tangent_on_side(1.0, CurveEvaluationSide::Left)
                .unwrap()
                .tangent()
                .as_vector()
                .to_array(),
            [left_x, 0.0, 0.0]
        );
        assert_eq!(
            view.evaluate_with_tangent_on_side(1.0, CurveEvaluationSide::Right)
                .unwrap()
                .tangent()
                .as_vector()
                .to_array(),
            [1.0, 0.0, 0.0]
        );
        let sampler = crate::curve::ArcLengthSampler::try_new(view, Tolerance::DEFAULT).unwrap();
        assert_eq!(sampler.kinks(1e-10).unwrap().len(), kink_count);
        assert!(
            (curve.kink_angle_at(1.0).unwrap()
                - if kink_count == 0 {
                    0.0
                } else {
                    std::f64::consts::PI
                })
            .abs()
                < 1e-15
        );
    }
}

#[test]
fn limiting_tangents_use_higher_derivatives_and_endpoint_orientation() {
    for degree in 2..=6 {
        for sign in [1.0, -1.0] {
            let mut controls = vec![(p(0.0, 0.0, 0.0), sign); degree + 1];
            controls[degree] = (p(1.0, 0.0, 0.0), 2.0 * sign);
            let mut knots = vec![0.0; degree + 1];
            knots.extend(vec![1.0; degree + 1]);
            let curve = rational(degree, &controls, knots);
            let reversed = curve.reversed().unwrap();
            for side in [CurveEvaluationSide::Left, CurveEvaluationSide::Right] {
                assert_eq!(
                    curve
                        .tangent_at_on_side(0.0, side)
                        .unwrap()
                        .as_vector()
                        .to_array(),
                    [1.0, 0.0, 0.0]
                );
                assert_eq!(
                    reversed
                        .tangent_at_on_side(0.0, side)
                        .unwrap()
                        .as_vector()
                        .to_array(),
                    [-1.0, 0.0, 0.0]
                );
            }
        }
    }
    let constant = rational(
        2,
        &[
            (p(3.0, 4.0, 0.0), 1.0),
            (p(3.0, 4.0, 0.0), 2.0),
            (p(3.0, 4.0, 0.0), 3.0),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    );
    for side in [CurveEvaluationSide::Left, CurveEvaluationSide::Right] {
        assert!(matches!(
            constant.tangent_at_on_side(0.5, side),
            Err(GeometryError::Degenerate { .. })
        ));
    }
}

#[test]
fn sided_first_derivatives_and_tangents_do_not_require_a_finite_second_derivative() {
    let curve = rational(
        1,
        &[(p(0.0, 0.0, 0.0), 1.0), (p(1.0, 0.0, 0.0), 2.0)],
        vec![0.0, 0.0, 1e-200, 1e-200],
    );
    let view = CurveRef::NurbsCurve(&curve);
    for side in [CurveEvaluationSide::Left, CurveEvaluationSide::Right] {
        for t in [0.0, 0.5e-200, 1e-200] {
            let (_, first) = view.evaluate_with_derivative_on_side(t, side).unwrap();
            assert!(first.x().is_finite() && first.x() > 0.0);
            assert_eq!(
                view.evaluate_with_tangent_on_side(t, side)
                    .unwrap()
                    .tangent()
                    .as_vector()
                    .to_array(),
                [1.0, 0.0, 0.0]
            );
            assert!(matches!(
                view.evaluate_with_second_derivative_on_side(t, side),
                Err(GeometryError::NonFinite { .. })
            ));
        }
    }
}
