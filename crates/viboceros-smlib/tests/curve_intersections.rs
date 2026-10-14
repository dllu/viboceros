#![cfg(feature = "native")]
use viboceros_geometry::{Circle3, UnitVector3};
use viboceros_geometry::{
    CurveCurveIntersectionEvent, NurbsCurve, Point3, Tolerance, WeightedPoint3,
};
use viboceros_smlib::curve_intersection_events;
fn curve(xy: &[[f64; 2]], domain: [f64; 2], gauge: f64, plane: usize) -> NurbsCurve {
    let degree = xy.len() - 1;
    let points = xy
        .iter()
        .map(|p| {
            let xyz = match plane {
                1 => [0., p[0], p[1]],
                2 => [p[1], 0., p[0]],
                _ => [p[0], p[1], 0.],
            };
            WeightedPoint3::try_new(Point3::try_from(xyz).unwrap(), gauge).unwrap()
        })
        .collect();
    NurbsCurve::try_new_rational(
        degree,
        points,
        [vec![domain[0]; degree + 1], vec![domain[1]; degree + 1]].concat(),
    )
    .unwrap()
}
#[test]
fn native_crossing_tangent_and_overlap_events_match_analytic_points_and_pair_parameters() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    for chart in 0..2 {
        for plane in 0..3 {
            for gauge in [1., 1e-280, -1e280] {
                for kind in 0..6 {
                    let domain = if chart == 0 { [0., 1.] } else { [-4., 7.] };
                    let other_domain = if chart == 0 { [0., 1.] } else { [10., 16.] };
                    let a = if kind == 1 || kind == 2 {
                        curve(&[[0., 0.], [1., 2.], [2., 0.]], domain, gauge, plane)
                    } else {
                        curve(&[[0., 0.], [2., 0.]], domain, gauge, plane)
                    };
                    let xy = match kind {
                        0 => [[1., -1.], [1., 1.]],
                        1 => [[0., 1.], [2., 1.]],
                        2 => [[0., 0.5], [2., 0.5]],
                        3 => [[0.5, 0.], [3., 0.]],
                        4 => [[3., 0.], [0.5, 0.]],
                        _ => [[0., 2e-8], [2., 2e-8]],
                    };
                    let b = curve(&xy, other_domain, gauge, plane);
                    let before = (a.clone(), b.clone());
                    let events = curve_intersection_events(&a, &b, tolerance).unwrap();
                    let expected_count = match kind {
                        2 => 2,
                        5 => 0,
                        _ => 1,
                    };
                    assert_eq!(
                        events.len(),
                        expected_count,
                        "kind {kind}, chart {chart}, plane {plane}, gauge {gauge}"
                    );
                    for (i, event) in events.iter().enumerate() {
                        match event {
                            CurveCurveIntersectionEvent::Point(point) => {
                                assert!(kind < 3);
                                let u = if kind < 2 {
                                    0.5
                                } else if i == 0 {
                                    (1. - 0.5_f64.sqrt()) * 0.5
                                } else {
                                    (1. + 0.5_f64.sqrt()) * 0.5
                                };
                                assert!(
                                    (point.first_parameter() - a.parameter_at(u).unwrap()).abs()
                                        < 1e-7
                                );
                                assert!(
                                    (point.second_parameter() - b.parameter_at(u).unwrap()).abs()
                                        < 1e-7
                                );
                                assert!(
                                    a.evaluate(point.first_parameter())
                                        .unwrap()
                                        .distance_to(point.point())
                                        .unwrap()
                                        <= tolerance.absolute()
                                );
                                assert!(
                                    b.evaluate(point.second_parameter())
                                        .unwrap()
                                        .distance_to(point.point())
                                        .unwrap()
                                        <= tolerance.absolute()
                                );
                            }
                            CurveCurveIntersectionEvent::Overlap(overlap) => {
                                assert!(kind == 3 || kind == 4);
                                assert!(
                                    (overlap.start().first_parameter()
                                        - a.parameter_at(0.25).unwrap())
                                    .abs()
                                        < 1e-10
                                );
                                assert_eq!(overlap.end().first_parameter(), domain[1]);
                                let (start, end) = if kind == 3 { (0., 0.6) } else { (1., 0.4) };
                                assert!(
                                    (overlap.start().second_parameter()
                                        - b.parameter_at(start).unwrap())
                                    .abs()
                                        < 1e-10
                                );
                                assert!(
                                    (overlap.end().second_parameter()
                                        - b.parameter_at(end).unwrap())
                                    .abs()
                                        < 1e-10
                                );
                            }
                        }
                    }
                    assert_eq!((a, b), before);
                }
            }
        }
    }
}

#[test]
fn rational_contacts_and_reversed_nonlinear_overlaps_preserve_original_images() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    for gauge in [1., 1e-280, -1e280] {
        let circle = NurbsCurve::try_new_rational(
            2,
            [
                ([1., 0., 0.], 1.),
                ([1., 1., 0.], 0.5_f64.sqrt()),
                ([0., 1., 0.], 1.),
            ]
            .into_iter()
            .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w * gauge).unwrap())
            .collect(),
            vec![-4., -4., -4., 7., 7., 7.],
        )
        .unwrap();
        let line = curve(&[[-1., 0.5], [1., 0.5]], [10., 16.], gauge, 0);
        let events = curve_intersection_events(&circle, &line, tolerance).unwrap();
        let [CurveCurveIntersectionEvent::Point(hit)] = events.as_slice() else {
            panic!("expected rational circle contact: {events:?}")
        };
        let expected = Point3::try_new(0.75_f64.sqrt(), 0.5, 0.).unwrap();
        assert!(hit.point().distance_to(expected).unwrap() <= tolerance.absolute());
        assert!(
            circle
                .evaluate(hit.first_parameter())
                .unwrap()
                .distance_to(expected)
                .unwrap()
                <= tolerance.absolute()
        );
        assert!(
            line.evaluate(hit.second_parameter())
                .unwrap()
                .distance_to(expected)
                .unwrap()
                <= tolerance.absolute()
        );
    }
    let a = curve(&[[0., 0.], [2., 0.]], [0., 1.], 1., 0);
    let b = curve(&[[2., 0.], [2., 0.], [0., 0.]], [-4., 7.], 1., 0);
    let events = curve_intersection_events(&a, &b, tolerance).unwrap();
    let [CurveCurveIntersectionEvent::Overlap(overlap)] = events.as_slice() else {
        panic!("expected nonlinear overlap: {events:?}")
    };
    assert_eq!(overlap.start().first_parameter(), 0.);
    assert_eq!(overlap.end().first_parameter(), 1.);
    assert_eq!(overlap.start().second_parameter(), 7.);
    assert_eq!(overlap.end().second_parameter(), -4.);
}

#[test]
fn native_intersection_fallback_keeps_high_degree_mixed_weights_and_full_order_breaks() {
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    let degree18 = NurbsCurve::try_new(
        18,
        (0..=18)
            .map(|i| Point3::try_new(2. * i as f64 / 18., 0., 0.).unwrap())
            .collect(),
        [vec![-4.; 19], vec![7.; 19]].concat(),
    )
    .unwrap();
    let vertical = curve(&[[1., -1.], [1., 1.]], [10., 16.], 1., 0);
    assert_eq!(
        curve_intersection_events(&degree18, &vertical, tolerance).unwrap(),
        degree18
            .intersection_events_with_curve(&vertical, tolerance)
            .unwrap()
    );
    let mixed = NurbsCurve::try_new_rational(
        2,
        [
            ([-1., 0., 0.], 1.),
            ([0., 1., 0.], -0.2),
            ([1., 0., 0.], 1.),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w).unwrap())
        .collect(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let vertical = curve(&[[0., -2.], [0., 2.]], [10., 16.], 1., 0);
    assert_eq!(
        curve_intersection_events(&mixed, &vertical, tolerance).unwrap(),
        mixed
            .intersection_events_with_curve(&vertical, tolerance)
            .unwrap()
    );
    let broken = NurbsCurve::try_new(
        1,
        [-2., -1., 1., 2.]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        vec![0., 0., 0.5, 0.5, 1., 1.],
    )
    .unwrap();
    assert!(
        curve_intersection_events(&broken, &vertical, tolerance)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn native_circle_tangency_reports_one_contact_at_an_interior_multiple_knot() {
    let tolerance = Tolerance::DEFAULT;
    let n = UnitVector3::try_new(0., 0., 1., tolerance).unwrap();
    let circle = Circle3::try_new(Point3::try_new(0., 0., 0.).unwrap(), 5., n, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let line = curve(&[[-5., -10.], [-5., 10.]], [0., 20.], 1., 0);
    let events = curve_intersection_events(&circle, &line, tolerance).unwrap();
    assert_eq!(events.len(), 1, "{events:?}");
    let CurveCurveIntersectionEvent::Point(hit) = events[0] else {
        panic!("{events:?}")
    };
    assert!(
        hit.point()
            .distance_to(Point3::try_new(-5., 0., 0.).unwrap())
            .unwrap()
            <= tolerance.absolute()
    );
}

#[test]
fn native_search_keeps_near_tangent_roots_short_spans_and_unclamped_linear_precision() {
    let tolerance = Tolerance::try_new(1e-12, 1e-14, 1e-10).unwrap();
    let parabola = curve(&[[0., 0.], [1., 2.], [2., 0.]], [0., 1.], 1., 0);
    let height = 1. - 1e-10;
    let line = curve(&[[0., height], [2., height]], [0., 1.], 1., 0);
    let events = curve_intersection_events(&parabola, &line, tolerance).unwrap();
    assert_eq!(events.len(), 2, "{events:?}");
    for (i, event) in events.iter().enumerate() {
        let CurveCurveIntersectionEvent::Point(hit) = event else {
            panic!()
        };
        let expected =
            (1. + if i == 0 {
                -(1. - height).sqrt()
            } else {
                (1. - height).sqrt()
            }) * 0.5;
        assert!((hit.first_parameter() - expected).abs() < 1e-9);
    }
    let tolerance = Tolerance::DEFAULT;
    let short = NurbsCurve::try_new(
        1,
        [[0., 1., 0.], [1., 1., 0.], [2., -1., 0.], [3., -1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![0., 0., 0.5, 0.5000000001, 1., 1.],
    )
    .unwrap();
    let line = curve(&[[0., 0.], [3., 0.]], [0., 1.], 1., 0);
    let events = curve_intersection_events(&short, &line, tolerance).unwrap();
    let [CurveCurveIntersectionEvent::Point(hit)] = events.as_slice() else {
        panic!("{events:?}")
    };
    assert!(
        hit.point()
            .distance_to(Point3::try_new(1.5, 0., 0.).unwrap())
            .unwrap()
            <= tolerance.absolute()
    );
    let unclamped = NurbsCurve::try_new(
        2,
        [-0.5, 0.4, 1.4, 2.5]
            .map(|x| Point3::try_new(x, 0., 0.).unwrap())
            .to_vec(),
        vec![-2., -1., 0., 0.8, 2., 3., 4.],
    )
    .unwrap();
    let line = curve(&[[1., -1.], [1., 1.]], [10., 16.], 1., 0);
    let events = curve_intersection_events(&unclamped, &line, tolerance).unwrap();
    let [CurveCurveIntersectionEvent::Point(hit)] = events.as_slice() else {
        panic!("{events:?}")
    };
    assert!((hit.first_parameter() - 1.).abs() < 1e-10);
    assert_eq!(hit.point(), Point3::try_new(1., 0., 0.).unwrap());
}

#[test]
fn native_off_center_polynomial_tangencies_are_polished_in_source_space() {
    let tolerance = Tolerance::DEFAULT;
    for curvature in [1., 0.0625] {
        let center = 0.3;
        let source = curve(
            &[
                [0., curvature * center * center],
                [0.5, curvature * (center * center - center)],
                [1., curvature * (1. - center) * (1. - center)],
            ],
            [0., 1.],
            1.,
            0,
        );
        let line = curve(&[[0., 0.], [1., 0.]], [0., 1.], 1., 0);
        let events = curve_intersection_events(&source, &line, tolerance).unwrap();
        let [CurveCurveIntersectionEvent::Point(hit)] = events.as_slice() else {
            panic!("{curvature}: {events:?}")
        };
        assert!(
            hit.point()
                .distance_to(Point3::try_new(center, 0., 0.).unwrap())
                .unwrap()
                <= tolerance.absolute(),
            "{curvature}: {hit:?}"
        );
    }
}
