#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::{split_curve, split_curve_at_parameters, subcurve};

fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(p["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    p.get("weight").map_or(1., |w| w.as_f64().unwrap()),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(value["knots"].clone()).unwrap(),
    )
    .unwrap()
}

fn compare(actual: &NurbsCurve, expected: &NurbsCurve) {
    assert_eq!(actual.degree(), expected.degree());
    assert_eq!(actual.knots(), expected.knots());
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

#[test]
fn native_directed_subcurves_preserve_reversal_closed_seams_and_source_definitions() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_subcurve.json"
    ))
    .unwrap();
    for op in fixture["operations"].as_array().unwrap() {
        let source = curve(&op["curve"]);
        let before = source.clone();
        let a = op["start"].as_f64().unwrap();
        let b = op["end"].as_f64().unwrap();
        compare(
            &subcurve(&source, a, b).unwrap(),
            &source.try_subcurve(a, b).unwrap(),
        );
        assert_eq!(source, before);
    }
}

#[test]
fn native_splits_preserve_open_order_and_closed_cyclic_seam_assembly() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/curve_split.json"
    ))
    .unwrap();
    for op in fixture["operations"].as_array().unwrap() {
        let source = curve(&op["curve"]);
        let before = source.clone();
        if let Some(t) = op.get("parameter") {
            let t = t.as_f64().unwrap();
            let actual = split_curve(&source, t).unwrap();
            let expected = source.try_split(t).unwrap();
            compare(&actual.0, &expected.0);
            compare(&actual.1, &expected.1);
        } else {
            let cuts = serde_json::from_value::<Vec<f64>>(op["parameters"].clone()).unwrap();
            let actual = split_curve_at_parameters(&source, &cuts).unwrap();
            let expected = source.try_split_at_parameters(&cuts).unwrap();
            assert_eq!(actual.len(), expected.len());
            for (a, e) in actual.iter().zip(&expected) {
                compare(a, e);
            }
        }
        assert_eq!(source, before);
    }
}

#[test]
fn full_order_splits_keep_independent_sides_and_exact_payloads() {
    let source = NurbsCurve::try_new_rational(
        2,
        (0..6)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new(i as f64, (i % 3) as f64, 0.).unwrap(),
                    if i < 3 {
                        0.7 + i as f64 / 10.
                    } else {
                        1.3 + i as f64 / 10.
                    },
                )
                .unwrap()
            })
            .collect(),
        vec![0., 0., 0., 0.5, 0.5, 0.5, 1., 1., 1.],
    )
    .unwrap();
    let (left, right) = split_curve(&source, 0.5).unwrap();
    assert_eq!(left.control_points(), &source.control_points()[..3]);
    assert_eq!(right.control_points(), &source.control_points()[3..]);
    assert_ne!(left.evaluate(0.5).unwrap(), right.evaluate(0.5).unwrap());
    let before = source.clone();
    assert!(subcurve(&source, 0.5, 0.5).is_err());
    for cuts in [vec![], vec![0.], vec![1.], vec![0.5, 0.5], vec![f64::NAN]] {
        assert!(split_curve_at_parameters(&source, &cuts).is_err());
    }
    assert_eq!(source, before);
}
