#![cfg(feature = "native")]
use viboceros_geometry::{
    Circle3, CurveCurveIntersectionEvent, NurbsCurve, Point3, Tolerance, UnitVector3,
    WeightedPoint3,
};
use viboceros_smlib::curve_self_intersection_events;

#[test]
fn distinct_visit_contacts_preserve_domains_planes_gauges_and_sources() {
    let inputs = [
        vec![[-1., 0.], [3., 2.], [-3., 2.], [1., 0.]],
        vec![[0., 0.], [1., 2.], [-1., 2.], [0., 0.]],
        vec![[0., 0.], [2., 2.], [0., 2.], [2., 0.]],
        vec![[0., 0.], [2., 0.], [1., 0.], [1., 2.]],
        vec![[0., 0.], [2., 0.], [2., 2.], [0., 2.], [0., 0.]],
        vec![[0., 0.], [2., 0.]],
    ];
    let tolerance = Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap();
    for (kind, input) in inputs.iter().enumerate() {
        for domain in [
            [0., 1.],
            [-4., 7.],
            [-2_f64.powi(360), 2_f64.powi(360)],
            [-2_f64.powi(-360), 2_f64.powi(-360)],
        ] {
            for plane in 0..3 {
                for gauge in [1., 1e-280, -1e280] {
                    let degree = if kind < 2 { 3 } else { 1 };
                    let mut knots = vec![domain[0]; degree + 1];
                    for i in 1..input.len() - degree {
                        knots.push(
                            domain[0] * (1. - i as f64 / (input.len() - degree) as f64)
                                + domain[1] * i as f64 / (input.len() - degree) as f64,
                        );
                    }
                    knots.extend(vec![domain[1]; degree + 1]);
                    let controls = input
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
                    let source = NurbsCurve::try_new_rational(degree, controls, knots).unwrap();
                    let before = source.clone();
                    let events = curve_self_intersection_events(&source, tolerance)
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        events.len(),
                        usize::from(matches!(kind, 0 | 2 | 3)),
                        "kind {kind}, plane {plane}, domain {domain:?}"
                    );
                    for event in events {
                        let contacts = match event {
                            CurveCurveIntersectionEvent::Point(point) => {
                                assert!(kind == 0 || kind == 2);
                                vec![point]
                            }
                            CurveCurveIntersectionEvent::Overlap(overlap) => {
                                assert_eq!(kind, 3);
                                let a = overlap.first_interval();
                                let b = overlap.second_interval();
                                assert!(*a.end() <= *b.start());
                                vec![overlap.start(), overlap.end()]
                            }
                        };
                        for (end, point) in contacts.into_iter().enumerate() {
                            let expected = match kind {
                                0 => [(1. - 0.6_f64.sqrt()) * 0.5, (1. + 0.6_f64.sqrt()) * 0.5],
                                2 => [1. / 6., 5. / 6.],
                                _ => {
                                    if end == 0 {
                                        [1. / 6., 2. / 3.]
                                    } else {
                                        [1. / 3., 1. / 3.]
                                    }
                                }
                            };
                            let a = (point.first_parameter() - domain[0]) / (domain[1] - domain[0]);
                            let b =
                                (point.second_parameter() - domain[0]) / (domain[1] - domain[0]);
                            assert!(
                                (a - expected[0]).abs() < 1e-9 && (b - expected[1]).abs() < 1e-9,
                                "{kind}: {a}, {b}"
                            );
                            assert!(
                                source
                                    .evaluate(point.first_parameter())
                                    .unwrap()
                                    .distance_to(source.evaluate(point.second_parameter()).unwrap())
                                    .unwrap()
                                    <= tolerance.absolute()
                            );
                        }
                    }
                    assert_eq!(source, before);
                }
            }
        }
    }
}

#[test]
fn simple_rational_circle_has_no_self_contacts() {
    let circle = Circle3::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        2.,
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    assert!(
        curve_self_intersection_events(&circle, Tolerance::DEFAULT)
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn projected_cubic_crossing_with_distinct_heights_is_not_a_spatial_contact() {
    let source = NurbsCurve::try_new(
        3,
        [
            [-1., 0., 0.],
            [3., 2., 1. / 3.],
            [-3., 2., 2. / 3.],
            [1., 0., 1.],
        ]
        .into_iter()
        .map(|p| Point3::try_from(p).unwrap())
        .collect(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    assert!(
        curve_self_intersection_events(&source, Tolerance::DEFAULT)
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn high_degree_declines_and_poles_are_rejected_before_native_search() {
    let source = NurbsCurve::try_new(
        18,
        (0..=18)
            .map(|i| Point3::try_new(i as f64, 0., 0.).unwrap())
            .collect(),
        [vec![0.; 19], vec![1.; 19]].concat(),
    )
    .unwrap();
    assert!(
        curve_self_intersection_events(&source, Tolerance::DEFAULT)
            .unwrap()
            .is_none()
    );
    let pole = NurbsCurve::try_new_rational(
        1,
        vec![
            WeightedPoint3::try_new(Point3::try_new(0., 0., 0.).unwrap(), 1.).unwrap(),
            WeightedPoint3::try_new(Point3::try_new(1., 0., 0.).unwrap(), -1.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    assert!(curve_self_intersection_events(&pole, Tolerance::DEFAULT).is_err());
}

#[test]
fn several_collinear_visits_keep_nondegenerate_paired_ranges() {
    for xs in [
        vec![0., 3., 1., 4.],
        vec![0., 3., 1., 3.],
        vec![0., 2., 1., 3.],
        vec![0., 3., 2., 4.],
        vec![0., 3., 1., 2., 0.],
        vec![0., 3., 1., 2., 1.],
        vec![0., 3., 1., 4., 2., 5.],
    ] {
        let n = xs.len();
        let mut knots = vec![0., 0.];
        knots.extend((1..n - 1).map(|i| i as f64));
        knots.extend([(n - 1) as f64; 2]);
        let source = NurbsCurve::try_new(
            1,
            xs.into_iter()
                .map(|x| Point3::try_new(x, 0., 0.).unwrap())
                .collect(),
            knots,
        )
        .unwrap();
        let events = curve_self_intersection_events(&source, Tolerance::DEFAULT)
            .unwrap()
            .unwrap();
        assert!(!events.is_empty());
        for event in events {
            if let CurveCurveIntersectionEvent::Overlap(overlap) = event {
                assert!(overlap.start().first_parameter() <= overlap.start().second_parameter());
                assert!(overlap.end().first_parameter() <= overlap.end().second_parameter());
                assert!(*overlap.first_interval().start() < *overlap.first_interval().end());
                assert!(*overlap.second_interval().start() < *overlap.second_interval().end());
                for contact in [overlap.start(), overlap.end()] {
                    assert!(
                        source
                            .evaluate(contact.first_parameter())
                            .unwrap()
                            .distance_to(source.evaluate(contact.second_parameter()).unwrap())
                            .unwrap()
                            < 1e-8
                    );
                }
            }
        }
    }
}
