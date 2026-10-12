#![cfg(feature = "native")]
use viboceros_geometry::{GeometryError, NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::{Error, trim_curve};

fn source(chart: f64, gauge: f64) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        2,
        [
            ([-2., 1., 0.], 0.75),
            ([0., 4., 1.], 2.),
            ([5., -2., -1.], 0.5),
            ([8., 3., 2.], 1.25),
        ]
        .into_iter()
        .map(|(p, w)| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), w * gauge).unwrap())
        .collect(),
        [-2., -1., 0., 0.8, 2., 3., 4.].map(|k| k * chart).to_vec(),
    )
    .unwrap()
}

fn compare(source: &NurbsCurve, start: f64, end: f64) {
    let before = source.clone();
    let actual = trim_curve(source, start..=end).unwrap();
    let expected = source.try_trimmed(start..=end).unwrap();
    assert_eq!(actual.knots(), expected.knots());
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(
        actual.control_points().len(),
        expected.control_points().len()
    );
    for (a, e) in actual
        .control_points()
        .iter()
        .zip(expected.control_points())
    {
        assert!(a.point().distance_to(e.point()).unwrap() < 1e-10);
        assert!((a.weight() / e.weight() - 1.).abs() < 1e-12);
    }
    for i in 0..=32 {
        let t = actual.parameter_at(i as f64 / 32.).unwrap();
        assert!(
            actual
                .evaluate(t)
                .unwrap()
                .distance_to(expected.evaluate(t).unwrap())
                .unwrap()
                < 1e-10
        );
    }
    assert_eq!(*source, before);
}

#[test]
fn native_segments_preserve_rational_shape_knots_charts_gauges_and_source() {
    for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
        for gauge in [1e-300, 1., 1e300, -1e300] {
            let source = source(chart, gauge);
            for [a, b] in [
                [0.25, 1.6],
                [0., 1.6],
                [0.25, 2.],
                [0.25, 0.6],
                [0.8, 1.6],
                [0., 2.],
            ] {
                compare(&source, a * chart, b * chart);
            }
        }
    }
}

#[test]
fn full_order_interior_breaks_and_inactive_endpoint_controls_remain_independent() {
    for knots in [
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
        vec![-2., 0., 0., 0., 0.5, 1., 2., 3.],
    ] {
        let source = NurbsCurve::try_new_rational(
            2,
            (0..knots.len() - 3)
                .map(|i| {
                    WeightedPoint3::try_new(
                        Point3::try_new(i as f64, (i % 3) as f64, 0.).unwrap(),
                        0.7 + i as f64 / 4.,
                    )
                    .unwrap()
                })
                .collect(),
            knots,
        )
        .unwrap();
        for curve in [source.clone(), source.reversed().unwrap()] {
            for [a, b] in [[0., 0.5], [0.25, 0.5], [0.5, 1.], [0.25, 0.75]] {
                let a = curve.parameter_at(a).unwrap();
                let b = curve.parameter_at(b).unwrap();
                compare(&curve, a, b);
            }
        }
    }
}

#[test]
fn whole_domain_noops_exact_line_crossings_and_invalid_requests_keep_existing_policy() {
    let source = source(1., 1.);
    assert_eq!(trim_curve(&source, source.domain()).unwrap(), source);
    for interval in [f64::NAN..=1., 0.5..=0.5, 1.5..=0.5, -1.0..=1.0, 0.0..=3.0] {
        assert!(matches!(
            trim_curve(&source, interval),
            Err(Error::Geometry(GeometryError::InvalidCurveTrimInterval))
        ));
    }
    let line = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(-1., -2., 7.).unwrap(),
            Point3::try_new(2., 4., 7.).unwrap(),
        ],
        vec![1e9, 1e9, 1e9 + 3., 1e9 + 3.],
    )
    .unwrap();
    let result = trim_curve(&line, (1e9 + 1.0)..=(1e9 + 2.0)).unwrap();
    assert_eq!(
        result.control_points()[0].point(),
        Point3::try_new(0., 0., 7.).unwrap()
    );
    assert_eq!(
        result.control_points()[1].point(),
        Point3::try_new(1., 2., 7.).unwrap()
    );
}

#[test]
fn recorded_rhino_trims_match_controls_and_loci_with_the_whole_domain_padding_difference_retained()
{
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_trim_geometry.json"
    ))
    .unwrap();
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_trim_geometry.json"
    ))
    .unwrap();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 9);
    for (op, record) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let c = &op["curve"];
        let source = NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            support::points(&c["control_points"]),
            support::knots(&c["knots"]),
        )
        .unwrap();
        let actual = trim_curve(
            &source,
            op["start"].as_f64().unwrap()..=op["end"].as_f64().unwrap(),
        )
        .unwrap();
        let e = &record["value"];
        let expected = NurbsCurve::try_new_rational(
            e["degree"].as_u64().unwrap() as usize,
            support::points(&e["control_points"]),
            support::knots(&e["knots"]),
        )
        .unwrap();
        if op["id"] == "trim-rational-whole" {
            assert_eq!(actual, source);
            assert_eq!(
                actual.knots()[1..actual.knots().len() - 1],
                expected.knots()[1..expected.knots().len() - 1]
            );
            assert_eq!((actual.knots()[0], expected.knots()[0]), (-2., -1.));
            assert_eq!(
                (
                    *actual.knots().last().unwrap(),
                    *expected.knots().last().unwrap()
                ),
                (4., 3.)
            );
            assert_eq!(actual.control_points(), expected.control_points());
        } else {
            support::compare(
                &support::curve_value(&actual),
                e,
                op["id"].as_str().unwrap(),
            );
        }
        for i in 0..=64 {
            let t = actual.parameter_at(i as f64 / 64.).unwrap();
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
