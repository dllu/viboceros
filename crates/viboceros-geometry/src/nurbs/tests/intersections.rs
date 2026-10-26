use super::*;

#[test]
fn curve_intersection_refines_transverse_nurbs_spans() {
    let horizontal = NurbsCurve::try_new(
        1,
        vec![point(-2.0, 1.0), point(8.0, 1.0)],
        vec![0.0, 0.0, 10.0, 10.0],
    )
    .unwrap();
    let vertical = NurbsCurve::try_new(
        1,
        vec![point(3.0, -4.0), point(3.0, 6.0)],
        vec![-5.0, -5.0, 5.0, 5.0],
    )
    .unwrap();
    let intersections = horizontal
        .intersections_with_curve(&vertical, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(intersections.len(), 1);
    assert_point_near(intersections[0].point(), point(3.0, 1.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[0].first_parameter(), 5.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[0].second_parameter(), 0.0));
}

#[test]
fn curve_intersection_finds_an_interior_tangent_contact() {
    let parabola = NurbsCurve::try_new(
        2,
        vec![point(0.0, 0.0), point(5.0, 8.0), point(10.0, 0.0)],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let tangent = NurbsCurve::try_new(
        1,
        vec![point(0.0, 4.0), point(10.0, 4.0)],
        vec![0.0, 0.0, 10.0, 10.0],
    )
    .unwrap();
    let intersections = parabola
        .intersections_with_curve(&tangent, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(intersections.len(), 1, "{intersections:#?}");
    assert_point_near(intersections[0].point(), point(5.0, 4.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[0].first_parameter(), 0.5));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[0].second_parameter(), 5.0));
}

#[test]
fn curve_intersection_refines_an_off_center_rational_tangency() {
    let arc = NurbsCurve::try_new_rational(
        2,
        vec![
            WeightedPoint3::try_new(point(1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0), 0.5_f64.sqrt()).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0), 1.0).unwrap(),
        ],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let parameter = 0.3;
    let (contact, derivative) = arc.evaluate_with_derivative(parameter).unwrap();
    let tangent = derivative.normalized_nonzero().unwrap();
    let offset = tangent.as_vector().scaled(3.0).unwrap();
    let line = NurbsCurve::try_new(
        1,
        vec![
            contact.translated(offset.scaled(-1.0).unwrap()).unwrap(),
            contact.translated(offset).unwrap(),
        ],
        vec![-3.0, -3.0, 3.0, 3.0],
    )
    .unwrap();

    let intersections = arc
        .intersections_with_curve(&line, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(intersections.len(), 1, "{intersections:#?}");
    assert_point_near(intersections[0].point(), contact);
    assert!((intersections[0].first_parameter() - parameter).abs() < 1.0e-11);
    assert!(intersections[0].second_parameter().abs() < 1.0e-11);
}

#[test]
fn curve_intersection_finds_a_rational_span_endpoint_tangent_to_a_polyline() {
    let weight = std::f64::consts::FRAC_1_SQRT_2;
    let circle = NurbsCurve::try_new_rational(
        2,
        [
            ([8.0, 5.0], 1.0),
            ([8.0, 6.5], weight),
            ([6.5, 6.5], 1.0),
            ([5.0, 6.5], weight),
            ([5.0, 5.0], 1.0),
            ([5.0, 3.5], weight),
            ([6.5, 3.5], 1.0),
            ([8.0, 3.5], weight),
            ([8.0, 5.0], 1.0),
        ]
        .into_iter()
        .map(|(coordinates, weight)| {
            WeightedPoint3::try_new(point(coordinates[0], coordinates[1]), weight).unwrap()
        })
        .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0],
    )
    .unwrap();
    let polygon = NurbsCurve::try_new(
        1,
        vec![
            point(2.0, 3.0),
            point(5.0, 3.0),
            point(5.0, 7.0),
            point(2.0, 7.0),
            point(2.0, 3.0),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
    )
    .unwrap();

    let circle_first = circle
        .intersections_with_curve(&polygon, Tolerance::DEFAULT)
        .unwrap();
    let polygon_first = polygon
        .intersections_with_curve(&circle, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(circle_first.len(), 1, "{circle_first:#?}");
    assert_eq!(polygon_first.len(), 1, "{polygon_first:#?}");
    assert!((circle_first[0].first_parameter() - 2.0).abs() < 1.0e-10);
    assert!((circle_first[0].second_parameter() - 1.5).abs() < 1.0e-10);
    assert!((polygon_first[0].first_parameter() - 1.5).abs() < 1.0e-10);
    assert!((polygon_first[0].second_parameter() - 2.0).abs() < 1.0e-10);
    assert_point_near(circle_first[0].point(), point(5.0, 5.0));
    assert_point_near(polygon_first[0].point(), point(5.0, 5.0));

    let separated = NurbsCurve::try_new(
        1,
        vec![
            point(2.0, 3.0),
            point(5.0 - 1.0e-8, 3.0),
            point(5.0 - 1.0e-8, 7.0),
            point(2.0, 7.0),
            point(2.0, 3.0),
        ],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
    )
    .unwrap();
    assert!(
        circle
            .intersections_with_curve(&separated, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    assert!(
        separated
            .intersections_with_curve(&circle, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn curve_intersection_certifies_nested_rational_arcs_are_disjoint() {
    let quarter_circle = |radius: Real| {
        NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(5.0 + radius, 5.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(5.0 + radius, 5.0 + radius), 0.5_f64.sqrt()).unwrap(),
                WeightedPoint3::try_new(point(5.0, 5.0 + radius), 1.0).unwrap(),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap()
    };
    let outer = quarter_circle(3.0);
    let inner = quarter_circle(1.5);
    let outer_node = CurveIntersectionNode::try_new(outer.clone(), 0).unwrap();
    let inner_node = CurveIntersectionNode::try_new(inner.clone(), 0).unwrap();

    assert!(
        curve_nodes_are_certifiably_disjoint(
            &outer_node,
            &inner_node,
            Tolerance::DEFAULT.absolute(),
        )
        .unwrap()
    );
    assert!(
        outer
            .intersections_with_curve(&inner, Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn curve_intersection_finds_two_tangencies_in_one_span_pair() {
    let double_contact = NurbsCurve::try_new(
        4,
        vec![
            point(0.0, 0.441),
            point(0.25, -0.609),
            point(0.5, 0.707_666_666_666_666_7),
            point(0.75, -0.609),
            point(1.0, 0.441),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let line = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(1.0, 0.0)],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();

    let intersections = double_contact
        .intersections_with_curve(&line, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(intersections.len(), 2, "{intersections:#?}");
    assert!((intersections[0].first_parameter() - 0.3).abs() < 1.0e-10);
    assert!((intersections[1].first_parameter() - 0.7).abs() < 1.0e-10);
    assert_point_near(intersections[0].point(), point(0.3, 0.0));
    assert_point_near(intersections[1].point(), point(0.7, 0.0));
}

#[test]
fn curve_intersection_returns_collinear_overlap_endpoints() {
    let long = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(20.0, 0.0)],
        vec![0.0, 0.0, 20.0, 20.0],
    )
    .unwrap();
    let short = NurbsCurve::try_new(
        1,
        vec![point(10.0, 0.0), point(15.0, 0.0)],
        vec![-2.0, -2.0, 3.0, 3.0],
    )
    .unwrap();
    let intersections = long
        .intersections_with_curve(&short, Tolerance::DEFAULT)
        .unwrap();
    assert_eq!(intersections.len(), 2, "{intersections:#?}");
    assert_point_near(intersections[0].point(), point(10.0, 0.0));
    assert_point_near(intersections[1].point(), point(15.0, 0.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[0].first_parameter(), 10.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[1].first_parameter(), 15.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[0].second_parameter(), -2.0));
    assert!(Tolerance::DEFAULT.approx_eq(intersections[1].second_parameter(), 3.0));
}

#[test]
fn curve_intersection_events_preserve_partial_overlap_intervals() {
    let first = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(10.0, 0.0)],
        vec![0.0, 0.0, 10.0, 10.0],
    )
    .unwrap();
    let second = NurbsCurve::try_new(
        1,
        vec![point(5.0, 0.0), point(15.0, 0.0)],
        vec![20.0, 20.0, 30.0, 30.0],
    )
    .unwrap();

    let events = first
        .intersection_events_with_curve(&second, Tolerance::DEFAULT)
        .unwrap();
    let [CurveCurveIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
        panic!("expected one overlap event, got {events:#?}")
    };
    assert_eq!(overlap.first_interval(), 5.0..=10.0);
    assert_eq!(overlap.second_interval(), 20.0..=25.0);
    assert_point_near(overlap.start().point(), point(5.0, 0.0));
    assert_point_near(overlap.end().point(), point(10.0, 0.0));
}

#[test]
fn curve_intersection_events_merge_adjacent_overlap_spans() {
    let first = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)],
        vec![0.0, 0.0, 5.0, 10.0, 10.0],
    )
    .unwrap();
    let second = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)],
        vec![20.0, 20.0, 25.0, 30.0, 30.0],
    )
    .unwrap();

    let events = first
        .intersection_events_with_curve(&second, Tolerance::DEFAULT)
        .unwrap();
    let [CurveCurveIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
        panic!("expected one merged overlap event, got {events:#?}")
    };
    assert_eq!(overlap.first_interval(), 0.0..=10.0);
    assert_eq!(overlap.second_interval(), 20.0..=30.0);
}

#[test]
fn curve_intersection_events_keep_a_collinear_endpoint_as_a_point() {
    let first = NurbsCurve::try_new(
        1,
        vec![point(0.0, 0.0), point(5.0, 0.0)],
        vec![0.0, 0.0, 5.0, 5.0],
    )
    .unwrap();
    let second = NurbsCurve::try_new(
        1,
        vec![point(5.0, 0.0), point(10.0, 0.0)],
        vec![5.0, 5.0, 10.0, 10.0],
    )
    .unwrap();

    let events = first
        .intersection_events_with_curve(&second, Tolerance::DEFAULT)
        .unwrap();
    let [CurveCurveIntersectionEvent::Point(intersection)] = events.as_slice() else {
        panic!("expected one endpoint event, got {events:#?}")
    };
    assert_point_near(intersection.point(), point(5.0, 0.0));
}
