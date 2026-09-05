use super::*;
use crate::{CircularArc3, CurveClosure, LineSegment, NurbsCurve};

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
    assert_eq!(
        &curve.segments()[0],
        &CurveRef::Arc(&arc).to_nurbs().unwrap()
    );
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
