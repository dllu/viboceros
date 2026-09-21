use super::*;
use crate::{CircularArc3, CurveClosure, LineSegment, NurbsCurve};
mod encodings;
mod search;

#[test]
fn endpoint_search_does_not_lose_nearby_points_because_of_a_distant_origin() {
    let endpoints = [-9_007_199_254_740_992.0, 1.0, 2.0]
        .into_iter()
        .enumerate()
        .map(|(curve, x)| Endpoint {
            curve,
            start: true,
            point: p(x, 0.0),
            outward_tangent: None,
        })
        .collect::<Vec<_>>();
    let found = find_candidates(
        &endpoints,
        CurveJoinOptions {
            tolerance: 1.0,
            preserve_direction: false,
            style: CurveJoinStyle::Batch,
        },
    )
    .unwrap();
    assert_eq!(
        found.iter().map(|c| (c.left, c.right)).collect::<Vec<_>>(),
        [(1, 2)]
    );
}

#[test]
fn distant_unrelated_input_does_not_prevent_nearby_curve_joining() {
    let inputs = [
        line(
            [-9_007_199_254_740_992.0, 0.0],
            [-9_007_199_254_740_992.0, 1e6],
        ),
        line([-10.0, 0.0], [1.0, 0.0]),
        line([2.0, 0.0], [10.0, 0.0]),
    ];
    let before = inputs.clone();
    let joined = join(&inputs, 1.0, false);
    assert_eq!(joined.len(), 2);
    assert_eq!(joined[0].source_indices(), &[1, 2]);
    assert_eq!(joined[1].source_indices(), &[0]);
    let Curve3::Polyline(curve) = joined[0].curve() else {
        panic!("linear chain");
    };
    assert_eq!(
        curve.vertices(),
        &[p(-10.0, 0.0), p(1.5, 0.0), p(10.0, 0.0)]
    );
    assert_eq!(curve.parameters(), &[0.0, 11.5, 20.0]);
    assert_eq!(inputs, before);
}

#[test]
fn merging_linear_leaves_preserves_parent_points_and_one_sided_derivatives() {
    use crate::{CurveSegment3, ParameterSide};
    let first = LineSegment::try_new(p(0., 0.), p(2., 0.), Tolerance::DEFAULT)
        .unwrap()
        .try_reparameterized(-7.0..=-3.0)
        .unwrap();
    let second = Polyline3::try_with_parameters(
        vec![p(2., 0.), p(2., 3.), p(5., 3.)],
        vec![10., 11., 14.],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let original = PolyCurve3::try_with_segment_domains(
        vec![CurveSegment3::Line(first), CurveSegment3::Polyline(second)],
        vec![100., 102., 110.],
    )
    .unwrap();
    let before = original.clone();
    let merged = assembly::merge_linear_runs(&original, Tolerance::DEFAULT).unwrap();
    assert_eq!(merged.segments().len(), 1);
    assert_eq!(merged.parameters(), &[100., 110.]);
    assert_eq!(merged.segments()[0].as_ref().domain(), -7.0..=3.0);
    for i in 0..=100 {
        for side in [ParameterSide::Left, ParameterSide::Right] {
            let t = 100. + i as f64 / 10.;
            let (a, da) = original.evaluate_with_derivative_on_side(t, side).unwrap();
            let (b, db) = merged.evaluate_with_derivative_on_side(t, side).unwrap();
            for (x, y) in a.to_array().into_iter().zip(b.to_array()) {
                assert!((x - y).abs() < 1e-12);
            }
            for (x, y) in da.to_array().into_iter().zip(db.to_array()) {
                assert!((x - y).abs() < 1e-12);
            }
        }
    }
    assert_eq!(original, before);
}

#[test]
fn seeded_cycle_retains_seed_parameters_and_prepend_seam() {
    let inputs = [
        line([0., 0.], [1., 0.]),
        line([1., 0.], [1., 2.]),
        line([1., 2.], [0., 0.]),
    ];
    let before = inputs.clone();
    let joined = join_curves(
        &inputs,
        CurveJoinOptions {
            tolerance: 0.01,
            preserve_direction: false,
            style: CurveJoinStyle::Seeded,
        },
        Tolerance::DEFAULT,
    )
    .unwrap();
    let Curve3::Polyline(curve) = joined[0].curve() else {
        panic!("linear cycle")
    };
    assert_eq!(
        curve.vertices(),
        &[p(1., 2.), p(0., 0.), p(1., 0.), p(1., 2.)]
    );
    assert_eq!(curve.parameters(), &[-5_f64.sqrt(), 0., 1., 3.]);
    assert_eq!(
        curve.evaluate(0.).unwrap(),
        inputs[0].as_ref().start_point().unwrap()
    );
    assert_eq!(
        curve.evaluate(1.).unwrap(),
        inputs[0].as_ref().end_point().unwrap()
    );
    assert_eq!(joined[0].source_indices(), &[0, 1, 2]);
    assert_eq!(inputs, before);
}

#[test]
fn join_rounds_subnormal_endpoint_midpoints_symmetrically() {
    let unit = Real::from_bits(1);
    for (left, right, expected) in [(1.0, 2.0, 2.0), (-1.0, 2.0, 0.0), (-31.0, -30.0, -30.0)] {
        let inputs = [
            line([0.0, 0.0], [1.0, left * unit]),
            line([1.0, right * unit], [2.0, 0.0]),
        ];
        for curves in [inputs.clone(), [inputs[1].clone(), inputs[0].clone()]] {
            let joined = join(&curves, 1e-6, false);
            assert_eq!(joined.len(), 1);
            let Curve3::Polyline(curve) = joined[0].curve() else {
                panic!("expected polyline")
            };
            assert_eq!(curve.vertices().len(), 3);
            assert_eq!(curve.vertices()[1], p(1.0, expected * unit));
        }
    }
}

#[test]
fn seeded_join_preserves_the_seed_domain_and_does_not_revisit_skipped_sources() {
    let inputs = [
        line([1.0, 0.0], [2.0, 0.0]),
        line([4.0, 0.0], [3.0, 0.0]),
        line([3.0, 0.0], [2.0, 0.0]),
    ];
    let options = CurveJoinOptions {
        tolerance: 0.01,
        preserve_direction: false,
        style: CurveJoinStyle::Seeded,
    };
    let result = join_curves(&inputs, options, Tolerance::DEFAULT).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].source_indices(), &[0, 2]);
    assert_eq!(
        result[0].curve().as_ref().start_point().unwrap(),
        p(1.0, 0.0)
    );
    let inputs = [
        line([1.0, 0.0], [2.0, 0.0]),
        line([0.0, 0.0], [1.0, 0.0]),
        line([3.0, 0.0], [2.0, 0.0]),
    ];
    let result = join_curves(&inputs, options, Tolerance::DEFAULT).unwrap();
    let Curve3::Polyline(curve) = result[0].curve() else {
        panic!("expected polyline")
    };
    assert_eq!(curve.parameters(), &[-1.0, 0.0, 1.0, 2.0]);
}

#[test]
fn seeded_join_does_not_start_a_second_chain_and_unrelated_curves_do_not_change_representation() {
    let mut inputs = vec![
        line([0., 0.], [1., 0.]),
        line([1., 0.], [2., 0.]),
        line([10., 0.], [11., 0.]),
        line([11., 0.], [12., 0.]),
    ];
    inputs.push(Curve3::NurbsCurve(
        NurbsCurve::try_clamped_uniform(2, vec![p(20., 0.), p(21., 1.), p(22., 0.)]).unwrap(),
    ));
    let before = inputs.clone();
    for style in [CurveJoinStyle::Seeded, CurveJoinStyle::Batch] {
        let joined = join_curves(
            &inputs,
            CurveJoinOptions {
                tolerance: 1e-6,
                preserve_direction: false,
                style,
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let chains = joined
            .iter()
            .filter(|c| c.source_indices().len() > 1)
            .collect::<Vec<_>>();
        assert_eq!(
            chains.len(),
            if style == CurveJoinStyle::Seeded {
                1
            } else {
                2
            }
        );
        assert!(
            chains
                .iter()
                .all(|c| matches!(c.curve(), Curve3::Polyline(_)))
        );
        assert_eq!(chains[0].source_indices(), [0, 1]);
        assert_eq!(inputs, before);
    }
}

#[test]
fn exact_and_extreme_tolerance_matching_scales_for_separated_vertical_data() {
    let curves = (0..5000)
        .map(|index| line([0.0, 3.0 * index as Real], [0.0, 3.0 * index as Real + 1.0]))
        .collect::<Vec<_>>();
    assert_eq!(join(&curves, 0.0, false).len(), curves.len());
    assert_eq!(join(&curves, 1e-300, false).len(), curves.len());
}

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}
fn line(start: [Real; 2], end: [Real; 2]) -> Curve3 {
    Curve3::Line(
        LineSegment::try_new(p(start[0], start[1]), p(end[0], end[1]), Tolerance::DEFAULT).unwrap(),
    )
}
fn join(curves: &[Curve3], tolerance: Real, preserve_direction: bool) -> Vec<JoinedCurve3> {
    join_curves(
        curves,
        CurveJoinOptions {
            style: CurveJoinStyle::Batch,
            tolerance,
            preserve_direction,
        },
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn branch_matching_prefers_straight_continuation_independent_of_input_order() {
    let curves = [
        line([0.0, 0.0], [0.0, 1.0]),
        line([-1.0, 0.0], [0.0, 0.0]),
        line([0.0, 0.0], [1.0, 0.0]),
    ];
    let joined = join(&curves, 0.0, false);
    assert_eq!(joined.len(), 2);
    assert_eq!(joined[0].source_indices(), &[1, 2]);
    assert_eq!(joined[1].source_indices(), &[0]);
    let Curve3::Polyline(curve) = joined[0].curve() else {
        panic!("expected polyline")
    };
    assert_eq!(curve.vertices(), &[p(-1.0, 0.0), p(0.0, 0.0), p(1.0, 0.0)]);
}

#[test]
fn chain_orientation_uses_majority_then_last_source_on_ties() {
    let inputs = [line([0.0, 0.0], [1.0, 0.0]), line([2.0, 0.0], [1.0, 0.0])];
    let joined = join(&inputs, 0.0, false);
    assert_eq!(joined.len(), 1);
    assert_eq!(
        joined[0].curve().as_ref().start_point().unwrap(),
        p(2.0, 0.0)
    );
    assert_eq!(join(&inputs, 0.0, true).len(), 2);
    let inputs = [
        line([1.0, 0.0], [2.0, 0.0]),
        line([0.0, 0.0], [1.0, 0.0]),
        line([3.0, 0.0], [2.0, 0.0]),
    ];
    assert_eq!(
        join(&inputs, 0.0, false)[0]
            .curve()
            .as_ref()
            .start_point()
            .unwrap(),
        p(0.0, 0.0)
    );
}

#[test]
fn flexible_junctions_move_to_midpoints_without_changing_source_intervals() {
    let curve = NurbsCurve::try_new(
        2,
        vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
        vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
    )
    .unwrap();
    let inputs = [
        Curve3::NurbsCurve(curve.clone()),
        line([1.004, 1.0], [1.004, 3.0]),
    ];
    let result = join(&inputs, 0.01, false);
    let Curve3::PolyCurve(joined) = result[0].curve() else {
        panic!("expected exact mixed curve")
    };
    assert_eq!(joined.parameters(), &[2.0, 5.0, 7.0]);
    assert_eq!(joined.segments()[0].evaluate(5.0).unwrap(), p(1.002, 1.0));
    assert_eq!(curve.evaluate(5.0).unwrap(), p(1.0, 1.0));
}

#[test]
fn arc_flexible_join_keeps_the_arc_but_two_arcs_retain_opposite_tangents() {
    let arc = CircularArc3::try_from_three_points(
        p(1.0, 0.0),
        p(
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2,
        ),
        p(0.0, 1.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let inputs = [Curve3::Arc(arc), line([0.004, 1.0], [-2.0, 1.0])];
    let result = join(&inputs, 0.01, false);
    let Curve3::PolyCurve(curve) = result[0].curve() else {
        panic!("expected exact mixed curve")
    };
    assert_eq!(&curve.segments()[0], &crate::CurveSegment3::Arc(arc));
    assert_eq!(
        curve.segments()[1]
            .evaluate(*curve.segments()[1].domain().start())
            .unwrap(),
        arc.end().unwrap()
    );
    let moved = arc
        .try_with_endpoints(None, Some(p(0.002, 1.0)), Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(moved.domain(), arc.domain());
    assert!(
        moved
            .start()
            .unwrap()
            .distance_to(arc.start().unwrap())
            .unwrap()
            < 1e-14
    );
    assert!(moved.end().unwrap().distance_to(p(0.002, 1.0)).unwrap() < 1e-14);
    assert!(
        CurveRef::Arc(&moved)
            .evaluate_with_tangent(0.0)
            .unwrap()
            .tangent()
            .as_vector()
            .dot(
                CurveRef::Arc(&arc)
                    .evaluate_with_tangent(0.0)
                    .unwrap()
                    .tangent()
                    .as_vector()
            )
            .unwrap()
            > 1.0 - 1e-14
    );
}

#[test]
fn degree_one_nurbs_are_joined_as_polylines_but_straight_nurbs_cannot_close() {
    let curve =
        NurbsCurve::try_new(1, vec![p(0.0, 0.0), p(1.0, 1.0)], vec![3.0, 3.0, 5.0, 5.0]).unwrap();
    let inputs = [Curve3::NurbsCurve(curve), line([1.0, 1.0], [3.0, 1.0])];
    let result = join(&inputs, 0.0, false);
    let Curve3::Polyline(curve) = result[0].curve() else {
        panic!("expected polyline")
    };
    assert_eq!(
        curve.parameters(),
        &[0.0, 2.0_f64.sqrt(), 2.0 + 2.0_f64.sqrt()]
    );
    assert_eq!(
        inputs[0].close(0.0, true, Tolerance::DEFAULT).unwrap().1,
        CurveClosure::NotClosable
    );
}

#[test]
fn cycles_and_disconnected_chains_use_every_source_once() {
    let inputs = [
        line([0.0, 0.0], [1.0, 0.0]),
        line([1.0, 1.0], [1.0, 0.0]),
        line([0.0, 0.0], [1.0, 1.0]),
        line([10.0, 0.0], [11.0, 0.0]),
    ];
    let joined = join(&inputs, 1e-9, false);
    assert_eq!(joined.len(), 2);
    assert!(joined[0].curve().as_ref().is_closed().unwrap());
    assert_eq!(
        joined
            .iter()
            .flat_map(|curve| curve.source_indices().iter().copied())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
}

#[test]
fn rejected_parameters_and_extreme_coordinate_fallback_are_bounded() {
    let inputs = [line([0.0, 0.0], [1.0, 0.0])];
    for tolerance in [-1.0, Real::NAN, Real::INFINITY] {
        assert!(matches!(
            join_curves(
                &inputs,
                CurveJoinOptions {
                    style: CurveJoinStyle::Batch,
                    tolerance,
                    preserve_direction: false
                },
                Tolerance::DEFAULT
            ),
            Err(GeometryError::InvalidCurveJoinTolerance)
        ));
    }
    assert!(join(&[], 0.0, false).is_empty());
    let distant = [
        line([-1e100, 0.0], [-1e100 + 1e90, 0.0]),
        line([1e100, 0.0], [1e100 + 1e90, 0.0]),
    ];
    assert_eq!(join(&distant, 1e-300, false).len(), 2);
    let excessive = vec![inputs[0].clone(); MAX_JOIN_INPUTS + 1];
    assert!(matches!(
        join_curves(
            &excessive,
            CurveJoinOptions {
                style: CurveJoinStyle::Batch,
                tolerance: 0.0,
                preserve_direction: false
            },
            Tolerance::DEFAULT
        ),
        Err(GeometryError::CurveJoinLimit {
            resource: "input curves",
            ..
        })
    ));
}
