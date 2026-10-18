#![cfg(feature = "native")]
use std::process::Command;
use std::sync::LazyLock;
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};
use viboceros_smlib::trim_curve;

static CASES: LazyLock<Vec<serde_json::Value>> = LazyLock::new(|| {
    let result = Command::new(env!("VIBOCEROS_NATIVE_CURVE_SEGMENT_PROBE"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(
        result.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let rows = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 512);
    rows
});

#[test]
fn native_curve_segments_preserve_limits_bases_controls_and_independent_allocation() {
    for row in CASES.iter() {
        assert_eq!(row["source_unchanged"], true, "{row}");
        assert_eq!(row["independent_ownership"], true, "{row}");
        for output in row["outputs"].as_array().unwrap() {
            assert_eq!(output["correct"], true, "{row}");
        }
    }
    eprintln!("CURVE_SEGMENT_LIMITS cases={}", CASES.len());
}

fn curve(value: &serde_json::Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["homogeneous"]
            .as_array()
            .unwrap()
            .chunks_exact(4)
            .map(|p| {
                let w = p[3].as_f64().unwrap();
                WeightedPoint3::try_new(
                    Point3::try_new(
                        p[0].as_f64().unwrap() / w,
                        p[1].as_f64().unwrap() / w,
                        p[2].as_f64().unwrap() / w,
                    )
                    .unwrap(),
                    w,
                )
                .unwrap()
            })
            .collect(),
        value["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn checked_segments_match_independent_limits_and_preserve_isolated_endpoint_payloads() {
    let mut isolated_bounds = 0;
    let mut whole_noops = 0;
    for row in CASES.iter() {
        let source = curve(&row["input"]);
        let before = source.clone();
        let expected = curve(&row["expected"]);
        let low = row["low"].as_f64().unwrap();
        let high = row["high"].as_f64().unwrap();
        let label = format!(
            "case={} chart={} rational={} gauge={}",
            row["case"], row["chart"], row["rational"], row["gauge"]
        );
        let actual =
            trim_curve(&source, low..=high).unwrap_or_else(|error| panic!("{label}: {error:?}"));
        assert_eq!(source, before, "{label}");
        if low == *source.domain().start() && high == *source.domain().end() {
            assert_eq!(actual, source, "{label}");
            whole_noops += 1;
        } else {
            assert_eq!(actual.degree(), expected.degree(), "{label}");
            assert_eq!(actual.knots(), expected.knots(), "{label}");
            assert_eq!(
                actual.control_points().len(),
                expected.control_points().len(),
                "{label}"
            );
            for (a, e) in actual
                .control_points()
                .iter()
                .zip(expected.control_points())
            {
                assert!(
                    a.point().distance_to(e.point()).unwrap() < 1e-10,
                    "{label}: {a:?} {e:?}"
                );
                assert!((a.weight() / e.weight() - 1.).abs() < 1e-12, "{label}");
            }
            for (t, start) in [(low, true), (high, false)] {
                if source.knots().iter().filter(|&&k| k == t).count() == source.degree() + 1 {
                    let first = source.knots().iter().position(|&k| k == t).unwrap();
                    let source_index = if start { first } else { first - 1 };
                    let point = if start {
                        actual.control_points().first()
                    } else {
                        actual.control_points().last()
                    }
                    .unwrap();
                    assert_eq!(point, &source.control_points()[source_index], "{label}");
                    isolated_bounds += 1;
                }
            }
        }
        for i in 0..=16 {
            let t = actual.parameter_at(i as f64 / 16.).unwrap();
            assert!(
                actual
                    .evaluate(t)
                    .unwrap()
                    .distance_to(expected.evaluate(t).unwrap())
                    .unwrap()
                    < 1e-10,
                "{label} at {t}"
            );
        }
    }
    assert!(isolated_bounds > 0 && whole_noops > 0);
    eprintln!(
        "CHECKED_SEGMENT_LIMITS cases={} isolated_bounds={isolated_bounds} whole_noops={whole_noops}",
        CASES.len()
    );
}
