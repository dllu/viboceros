#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{GeometryError, NurbsCurve, WeightedPoint3};
use viboceros_smlib::{Error, change_curve_seam};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_change_seam.json"
    ))
    .unwrap()
}

fn source(op: &Value) -> NurbsCurve {
    let c = &op["curve"];
    let mut curve = NurbsCurve::try_new_rational(
        c["degree"].as_u64().unwrap() as usize,
        support::points(&c["control_points"]),
        support::knots(&c["knots"]),
    )
    .unwrap();
    if let Some(domain) = c.get("domain") {
        curve = curve
            .try_reparameterized(domain[0].as_f64().unwrap()..=domain[1].as_f64().unwrap())
            .unwrap();
    }
    curve
}

fn compare(source: &NurbsCurve, parameter: f64) -> NurbsCurve {
    let before = source.clone();
    let actual = change_curve_seam(source, parameter).unwrap();
    let expected = source.try_change_closed_seam(parameter).unwrap();
    assert_eq!(actual.knots(), expected.knots());
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(actual.is_periodic(), expected.is_periodic());
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
    for i in 0..=64 {
        let t = actual.parameter_at(i as f64 / 64.).unwrap();
        let domain = source.domain();
        let old_t = if domain.contains(&t) {
            t
        } else {
            (t - domain.start()).rem_euclid(domain.end() - domain.start()) + domain.start()
        };
        assert!(
            actual
                .evaluate(t)
                .unwrap()
                .distance_to(expected.evaluate(t).unwrap())
                .unwrap()
                < 1e-10
        );
        assert!(
            actual
                .evaluate(t)
                .unwrap()
                .distance_to(source.evaluate(old_t).unwrap())
                .unwrap()
                < 1e-10
        );
    }
    assert_eq!(*source, before);
    actual
}

#[test]
fn native_seams_match_recorded_rhino_definitions_and_sampled_loci() {
    let fixture = fixture();
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/curve_change_seam.json"
    ))
    .unwrap();
    assert_eq!(fixture["operations"].as_array().unwrap().len(), 11);
    assert_eq!(reference["results"].as_array().unwrap().len(), 11);
    for (op, record) in fixture["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(reference["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], record["id"]);
        let actual = compare(&source(op), op["parameter"].as_f64().unwrap());
        support::compare(
            &support::curve_value(&actual),
            &record["value"],
            op["id"].as_str().unwrap(),
        );
    }
}

#[test]
fn native_seams_preserve_binary_charts_signed_gauges_and_outside_domain_policy() {
    let fixture = fixture();
    for op in fixture["operations"].as_array().unwrap() {
        let original = source(op);
        for chart in [2_f64.powi(-360), 1., 2_f64.powi(360)] {
            for gauge in [1e-300, 1., 1e300, -1e300] {
                let scaled = NurbsCurve::try_new_rational(
                    original.degree(),
                    original
                        .control_points()
                        .iter()
                        .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * gauge).unwrap())
                        .collect(),
                    original.knots().iter().map(|k| k * chart).collect(),
                )
                .unwrap();
                let p = op["parameter"].as_f64().unwrap() * chart;
                let period = scaled.domain().end() - scaled.domain().start();
                for parameter in [p, p + period, p - period] {
                    compare(&scaled, parameter);
                }
            }
        }
    }
}

#[test]
fn invalid_and_endpoint_seams_preserve_validation_and_exact_noops() {
    let fixture = fixture();
    let closed = source(&fixture["operations"][0]);
    assert_eq!(
        change_curve_seam(&closed, *closed.domain().start()).unwrap(),
        closed
    );
    assert!(matches!(
        change_curve_seam(&closed, f64::NAN),
        Err(Error::Geometry(_))
    ));
    let open = closed.try_trimmed(2.0..=3.0).unwrap();
    assert!(matches!(
        change_curve_seam(&open, 2.5),
        Err(Error::Geometry(GeometryError::CurveSeamMustBeClosed))
    ));
}

#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
