#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};

#[test]
fn native_natural_extension_preserves_saved_curve_domains_and_exact_exterior_geometry() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_extend.json"
    ))
    .unwrap();
    for op in request["operations"].as_array().unwrap() {
        let c = &op["curve"];
        let source = NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            c["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                        )
                        .unwrap(),
                        p["weight"].as_f64().unwrap_or(1.),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(c["knots"].clone()).unwrap(),
        )
        .unwrap();
        let interval = op["domain"][0].as_f64().unwrap()..=op["domain"][1].as_f64().unwrap();
        let expected = source.try_extended_to(interval.clone()).unwrap();
        let actual = viboceros_smlib::extend_curve(&source, interval)
            .unwrap_or_else(|e| panic!("{}: {e}", op["id"]));
        assert_eq!(actual.domain(), expected.domain(), "{}", op["id"]);
        assert_eq!(actual.knots(), expected.knots(), "{}", op["id"]);
        for i in 0..=64 {
            let t = expected.parameter_at(i as f64 / 64.).unwrap();
            let error = expected
                .evaluate(t)
                .unwrap()
                .distance_to(actual.evaluate(t).unwrap())
                .unwrap();
            assert!(error < 1e-10, "{} at {t}: {error}", op["id"]);
        }
    }
}

#[test]
fn native_extension_preserves_full_order_breaks_and_signed_extreme_gauges() {
    let source = NurbsCurve::try_new(
        2,
        (0..6)
            .map(|i| Point3::try_new(i as f64, (i % 3) as f64, 0.).unwrap())
            .collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    for gauge in [1., 1e-280, -1e280] {
        let source = NurbsCurve::try_new_rational(
            2,
            source
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                .collect(),
            source.knots().to_vec(),
        )
        .unwrap();
        let expected = source.try_extended_to(-0.25..=1.25).unwrap();
        let actual = viboceros_smlib::extend_curve(&source, -0.25..=1.25).unwrap();
        assert_eq!(actual.knots(), expected.knots());
        for i in 0..=64 {
            let t = expected.parameter_at(i as f64 / 64.).unwrap();
            assert!(
                expected
                    .evaluate(t)
                    .unwrap()
                    .distance_to(actual.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10
            );
        }
    }
}

#[test]
fn native_extension_has_independent_linear_and_quadratic_precision_through_degree_seventeen() {
    for degree in 1..=17 {
        let source = NurbsCurve::try_new(
            degree,
            (0..=degree)
                .map(|i| Point3::try_new(i as f64, (i * i) as f64, 0.).unwrap())
                .collect(),
            std::iter::repeat_n(0., degree + 1)
                .chain(std::iter::repeat_n(1., degree + 1))
                .collect(),
        )
        .unwrap();
        let actual = viboceros_smlib::extend_curve(&source, -0.25..=1.25).unwrap();
        for i in 0..=64 {
            let t = -0.25 + 1.5 * i as f64 / 64.;
            let p = actual.evaluate(t).unwrap();
            let expected = Point3::try_new(
                degree as f64 * t,
                degree as f64 * t + (degree * (degree - 1)) as f64 * t * t,
                0.,
            )
            .unwrap();
            let error = p.distance_to(expected).unwrap();
            assert!(error < 1e-9, "degree={degree} t={t} error={error}");
        }
    }
}

#[test]
fn native_arc_length_extension_preserves_the_shared_length_solve() {
    use viboceros_geometry::{CurveExtensionSide as S, Tolerance};
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_extend_length.json"
    ))
    .unwrap();
    for op in request["operations"].as_array().unwrap() {
        let c = &op["curve"];
        let source = NurbsCurve::try_new_rational(
            c["degree"].as_u64().unwrap() as usize,
            c["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    WeightedPoint3::try_new(
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                        )
                        .unwrap(),
                        p["weight"].as_f64().unwrap_or(1.),
                    )
                    .unwrap()
                })
                .collect(),
            serde_json::from_value(c["knots"].clone()).unwrap(),
        )
        .unwrap();
        let side = match op["side"].as_str().unwrap() {
            "start" => S::Start,
            "end" => S::End,
            "both" => S::Both,
            _ => unreachable!(),
        };
        let length = op["length"].as_f64().unwrap();
        let expected = source
            .try_extended_by_length(side, length, Tolerance::DEFAULT)
            .unwrap();
        let actual =
            viboceros_smlib::extend_curve_by_length(&source, side, length, Tolerance::DEFAULT)
                .unwrap();
        for (a, b) in [*actual.domain().start(), *actual.domain().end()]
            .into_iter()
            .zip([*expected.domain().start(), *expected.domain().end()])
        {
            assert!((a - b).abs() < 1e-10);
        }
        for i in 0..=64 {
            let t = actual.parameter_at(i as f64 / 64.).unwrap();
            let q = expected.parameter_at(i as f64 / 64.).unwrap();
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate(q).unwrap())
                    .unwrap()
                    < 1e-9,
                "{}",
                op["id"]
            );
        }
    }
}

#[test]
fn native_extension_conditions_extreme_affine_parameter_charts() {
    let base = NurbsCurve::try_new(
        2,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 1., 0.).unwrap(),
            Point3::try_new(2., 4., 0.).unwrap(),
        ],
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    for (origin, span) in [(0., 2_f64.powi(-800)), (2_f64.powi(810), 2_f64.powi(800))] {
        let source = base.try_reparameterized(origin..=origin + span).unwrap();
        let before = source.clone();
        let actual =
            viboceros_smlib::extend_curve(&source, origin - 0.25 * span..=origin + 1.25 * span)
                .unwrap();
        for i in 0..=64 {
            let t = -0.25 + 1.5 * i as f64 / 64.;
            let expected = Point3::try_new(2. * t, 2. * t + 2. * t * t, 0.).unwrap();
            assert!(
                actual
                    .evaluate(origin + t * span)
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
        }
        assert_eq!(source, before);
    }
}

#[test]
fn degree_eighteen_extension_retains_the_portable_fallback() {
    let source = NurbsCurve::try_new(
        18,
        (0..=18)
            .map(|i| Point3::try_new(i as f64, (i * i) as f64, 0.).unwrap())
            .collect(),
        std::iter::repeat_n(0., 19)
            .chain(std::iter::repeat_n(1., 19))
            .collect(),
    )
    .unwrap();
    assert_eq!(
        viboceros_smlib::extend_curve(&source, -0.25..=1.25).unwrap(),
        source.try_extended_to(-0.25..=1.25).unwrap()
    );
}
