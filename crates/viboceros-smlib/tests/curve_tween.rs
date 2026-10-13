#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    CurveTweenMatchMethod, GeometryError, NurbsCurve, Point3, Tolerance, try_tween_nurbs_curves,
};
use viboceros_smlib::{Error, curve_tweens};
fn curve(v: &Value) -> NurbsCurve {
    let c = NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        support::points(&v["control_points"]),
        support::knots(&v["knots"]),
    )
    .unwrap();
    if let Some(d) = v.get("domain") {
        c.try_reparameterized(d[0].as_f64().unwrap()..=d[1].as_f64().unwrap())
            .unwrap()
    } else {
        c
    }
}
fn compare(a: &NurbsCurve, b: &NurbsCurve) {
    support::compare(&support::curve_value(a), &support::curve_value(b), "tween");
    for i in 0..=128 {
        let t = a.parameter_at(i as f64 / 128.).unwrap();
        assert!(
            a.evaluate(t)
                .unwrap()
                .distance_to(b.evaluate(t).unwrap())
                .unwrap()
                < 1e-8
        );
    }
}
fn nonlinear(z: f64, tiny: f64) -> NurbsCurve {
    NurbsCurve::try_new(
        3,
        [
            [0., 0., z],
            [1., 2., z + tiny],
            [4., -1., z - tiny],
            [6., 0., z + tiny],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap()
}
#[test]
fn native_tween_curves_preserve_all_permanent_modes_and_short_sample_spacing() {
    let mut cases = 0;
    for f in [
        include_str!("../../../tools/rhino_oracle/fixtures/curve_tween.json"),
        include_str!("../../../tools/rhino_oracle/fixtures/curve_tween_short_samples.json"),
    ] {
        let d: Value = serde_json::from_str(f).unwrap();
        let tolerance = d.get("tolerance").map_or(Tolerance::DEFAULT, |t| {
            Tolerance::try_new(
                t["absolute"].as_f64().unwrap(),
                t["relative"].as_f64().unwrap(),
                t["angular"].as_f64().unwrap(),
            )
            .unwrap()
        });
        for op in d["operations"].as_array().unwrap() {
            let a = curve(&op["start_curve"]);
            let b = curve(&op["end_curve"]);
            let before = (a.clone(), b.clone());
            let number = op["number"].as_u64().unwrap() as usize;
            let method = match op["method"].as_str().unwrap() {
                "control_point" => CurveTweenMatchMethod::ControlPoint,
                "refit" => CurveTweenMatchMethod::Refit,
                "sample_points" => CurveTweenMatchMethod::SamplePoints {
                    sample_number: op["sample_number"].as_u64().unwrap() as usize,
                },
                _ => unreachable!(),
            };
            let actual = curve_tweens(&a, &b, number, method, tolerance).unwrap();
            let expected = try_tween_nurbs_curves(&a, &b, number, method, tolerance).unwrap();
            for (a, b) in actual.iter().zip(&expected) {
                compare(a, b);
            }
            assert_eq!((a, b), before);
            cases += 1;
        }
    }
    assert_eq!(cases, 7);
}
#[test]
fn native_sampled_tweens_cover_the_complete_division_range_and_large_bands() {
    let a = nonlinear(0., 0.);
    let b = nonlinear(2., 0.);
    for sample_number in [2, 3, 255, 256, 1000, 9999] {
        let method = CurveTweenMatchMethod::SamplePoints { sample_number };
        let actual = curve_tweens(&a, &b, 1, method, Tolerance::DEFAULT).unwrap();
        let expected = try_tween_nurbs_curves(&a, &b, 1, method, Tolerance::DEFAULT).unwrap();
        assert_eq!(actual[0].degree(), 3);
        assert_eq!(actual[0].control_points().len(), sample_number + 3);
        compare(&actual[0], &expected[0]);
    }
}
#[test]
fn native_large_sampled_tweens_keep_small_coordinate_channels_and_sources() {
    for tiny in [1e-18, 1e-180, 1e-280] {
        let a = nonlinear(0., tiny);
        let b = nonlinear(0., 2. * tiny);
        let before = (a.clone(), b.clone());
        let method = CurveTweenMatchMethod::SamplePoints {
            sample_number: 1000,
        };
        let actual = curve_tweens(&a, &b, 2, method, Tolerance::DEFAULT).unwrap();
        let expected = try_tween_nurbs_curves(&a, &b, 2, method, Tolerance::DEFAULT).unwrap();
        for (a, b) in actual.iter().zip(&expected) {
            for (a, b) in a.control_points().iter().zip(b.control_points()) {
                assert!((a.point().z() / tiny - b.point().z() / tiny).abs() < 1e-9);
            }
        }
        assert_eq!((a, b), before);
    }
}
#[test]
fn native_sampling_budgets_preserve_typed_errors() {
    let a = nonlinear(0., 0.);
    let b = nonlinear(2., 0.);
    assert!(matches!(
        curve_tweens(
            &a,
            &b,
            1,
            CurveTweenMatchMethod::SamplePoints {
                sample_number: 10000
            },
            Tolerance::DEFAULT
        ),
        Err(Error::Geometry(
            GeometryError::InvalidCurveTweenSampleCount { .. }
        ))
    ));
    assert!(matches!(
        curve_tweens(
            &a,
            &b,
            100,
            CurveTweenMatchMethod::SamplePoints {
                sample_number: 9999
            },
            Tolerance::DEFAULT
        ),
        Err(Error::Geometry(
            GeometryError::TooManyCurveTweenControlPoints { .. }
        ))
    ));
}
#[path = "support/nurbs.rs"]
#[allow(dead_code)]
mod support;
