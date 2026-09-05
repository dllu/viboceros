use super::*;

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn polyline(vertices: &[[Real; 2]]) -> Polyline3 {
    Polyline3::try_new(
        vertices.iter().map(|p| point(p[0], p[1])).collect(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn polyline_closure_preserves_native_parameters_and_representation() {
    let source = Curve3::Polyline(polyline(&[[0.0, 0.0], [3.0, 0.0], [3.0, 2.0]]));
    let (closed, outcome) = source.close(1e-9, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(outcome, CurveClosure::SegmentAdded);
    let Curve3::PolyCurve(curve) = &closed else {
        panic!("a closing line is a separate segment")
    };
    assert!(curve.is_closed().unwrap());
    assert_eq!(curve.segments().len(), 2);
    assert_eq!(curve.parameters(), &[0.0, 2.0, 2.0 + 13.0_f64.sqrt()]);
    let crate::CurveSegment3::Polyline(polyline) = &curve.segments()[0] else {
        panic!("the original polyline stays native")
    };
    assert_eq!(polyline.parameters(), &[0.0, 1.0, 2.0]);
    assert!(matches!(curve.segments()[1], crate::CurveSegment3::Line(_)));
    assert_eq!(
        source.close(1e-9, false, Tolerance::DEFAULT).unwrap(),
        (source.clone(), CurveClosure::GapTooWide)
    );
    assert_eq!(
        closed.close(1e-9, false, Tolerance::DEFAULT).unwrap(),
        (closed.clone(), CurveClosure::AlreadyClosed)
    );
}

#[test]
fn near_two_segment_polyline_falls_back_to_a_line_instead_of_rejecting() {
    let source = Curve3::Polyline(polyline(&[[0.0, 0.0], [1.0, 1.0], [0.002, 0.0]]));
    let (closed, outcome) = source.close(0.01, true, Tolerance::DEFAULT).unwrap();
    assert_eq!(outcome, CurveClosure::SegmentAdded);
    let Curve3::PolyCurve(curve) = closed else {
        panic!("expected composite")
    };
    assert_eq!(curve.parameters(), &[0.0, 2.0, 2.002]);
    assert!(curve.is_closed().unwrap());
}

#[test]
fn zero_tolerance_moves_flexible_endpoints_and_invalid_tolerances_fail() {
    let polyline = polyline(&[[0.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0]]);
    let source = Curve3::Polyline(polyline.clone());
    let (closed, outcome) = source.close(0.0, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(outcome, CurveClosure::EndpointMoved);
    let Curve3::Polyline(curve) = closed else {
        panic!("an endpoint move retains the representation")
    };
    assert!(curve.is_closed());
    assert_eq!(curve.parameters(), polyline.parameters());
    for invalid in [-1.0, Real::NAN, Real::INFINITY] {
        assert_eq!(
            source.close(invalid, true, Tolerance::DEFAULT),
            Err(GeometryError::InvalidCurveClosureTolerance)
        );
    }
    let line = Curve3::Polyline(super::tests::polyline(&[[0.0, 0.0], [1.0, 0.0]]));
    assert_eq!(
        line.close(0.0, true, Tolerance::DEFAULT).unwrap(),
        (line.clone(), CurveClosure::NotClosable)
    );
}

#[test]
fn zero_tolerance_completes_the_supporting_circle_but_keeps_the_arc_domain() {
    let arc = CircularArc3::try_from_three_points(
        point(1.0, 0.0),
        point(0.0, 1.0),
        point(-1.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = Curve3::Arc(arc);
    let (closed, outcome) = source.close(0.0, false, Tolerance::DEFAULT).unwrap();
    assert_eq!(outcome, CurveClosure::EndpointMoved);
    let Curve3::Arc(circle) = closed else {
        panic!("expected analytic circular curve")
    };
    assert_eq!(circle.domain(), arc.domain());
    assert_eq!(circle.point_at(1.0).unwrap(), circle.start().unwrap());
    assert!(CurveRef::Arc(&circle).is_closed().unwrap());
    assert_eq!(
        CurveRef::Arc(&circle).to_nurbs().unwrap().domain(),
        arc.domain()
    );
    assert_eq!(
        source.close(1e-9, true, Tolerance::DEFAULT).unwrap().1,
        CurveClosure::SegmentAdded
    );
}

#[test]
fn rational_endpoint_edits_preserve_weights_domain_and_refresh_bounds() {
    let controls = [
        (point(0.0, 0.0), 2.0),
        (point(1.0, 0.0), 0.5),
        (point(1.0, 1.0), 1.5),
        (point(0.002, 0.0), 3.0),
    ]
    .into_iter()
    .map(|(p, w)| WeightedPoint3::try_new(p, w).unwrap())
    .collect();
    let curve =
        NurbsCurve::try_new_rational(3, controls, vec![2.0, 2.0, 2.0, 2.0, 5.0, 5.0, 5.0, 5.0])
            .unwrap();
    let changed = curve
        .try_with_endpoints(Some(point(-4.0, 0.0)), Some(point(-4.0, 0.0)))
        .unwrap();
    assert_eq!(changed.knots(), curve.knots());
    assert_eq!(
        changed
            .control_points()
            .iter()
            .map(|p| p.weight())
            .collect::<Vec<_>>(),
        vec![2.0, 0.5, 1.5, 3.0]
    );
    assert_eq!(
        &changed.control_points()[1..3],
        &curve.control_points()[1..3]
    );
    assert_eq!(changed.evaluate(2.0).unwrap(), point(-4.0, 0.0));
    assert_eq!(changed.control_point_bounds().min().x(), -4.0);
    assert!(changed.is_closed().unwrap());
}

#[test]
fn nontrivial_unclamped_endpoint_edit_clamps_both_ends_but_noop_does_not() {
    let curve = NurbsCurve::try_new(
        2,
        vec![
            point(0.0, 0.0),
            point(1.0, 2.0),
            point(3.0, 2.0),
            point(4.0, 0.0),
        ],
        vec![-1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    let start = curve.evaluate(1.0).unwrap();
    assert_eq!(curve.try_with_endpoints(Some(start), None).unwrap(), curve);
    let changed = curve
        .try_with_endpoints(None, Some(point(5.0, 1.0)))
        .unwrap();
    assert_eq!(changed.domain(), curve.domain());
    assert_eq!(&changed.knots()[..3], &[1.0, 1.0, 1.0]);
    assert_eq!(
        &changed.knots()[changed.knots().len() - 3..],
        &[3.0, 3.0, 3.0]
    );
    assert_eq!(changed.evaluate(1.0).unwrap(), start);
    assert_eq!(changed.evaluate(3.0).unwrap(), point(5.0, 1.0));
}

#[test]
fn polyline_parameterized_evaluation_reversal_sampling_and_transform_agree() {
    let curve = Polyline3::try_with_parameters(
        vec![point(0.0, 0.0), point(2.0, 0.0), point(2.0, 4.0)],
        vec![7.0, 8.0, 12.0],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for t in [7.0, 7.5, 8.0, 10.0, 12.0] {
        assert_eq!(
            curve.evaluate(t).unwrap(),
            curve.to_native_nurbs().unwrap().evaluate(t).unwrap()
        );
        assert_eq!(
            curve.evaluate(t).unwrap(),
            curve.reversed().evaluate(-t).unwrap()
        );
        assert_eq!(
            CurveRef::Polyline(&curve)
                .evaluate_with_tangent(t)
                .unwrap()
                .point(),
            curve.evaluate(t).unwrap()
        );
    }
    let samples = CurveRef::Polyline(&curve)
        .divide_by_count_samples(3, true, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.parameter())
            .collect::<Vec<_>>(),
        vec![7.0, 8.0, 10.0, 12.0]
    );
    for parameters in [
        vec![0.0, 1.0],
        vec![0.0, 1.0, 1.0],
        vec![0.0, Real::NAN, 2.0],
        vec![-Real::MAX, Real::MAX, Real::MAX],
    ] {
        assert!(
            Polyline3::try_with_parameters(
                curve.vertices().to_vec(),
                parameters,
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}
