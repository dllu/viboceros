use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{LoftEndPoints, LoftStyle, NurbsCurve, NurbsSurface, Point3, Tolerance};
#[allow(dead_code)]
#[path = "loft_parameters.rs"]
mod definitions;

pub fn compare<E: std::fmt::Debug>(
    epsilon: f64,
    loft: impl Fn(
        &[NurbsCurve],
        LoftStyle,
        bool,
        LoftEndPoints,
        usize,
        Tolerance,
    ) -> Result<NurbsSurface, E>,
) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let mut maximum = 0_f64;
    let mut matched = 0;
    for family in ["loft_rebuild_smoke", "loft_rebuild", "loft_rebuild_bounds"] {
        let q: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            assert_eq!(row["value"]["before"], row["value"]["after"]);
            let curves = op["curves"]
                .as_array()
                .unwrap()
                .iter()
                .map(definitions::curve)
                .collect::<Vec<_>>();
            let before = curves.clone();
            let point = |key: &str| {
                op.get(key).map(|p| {
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap())
                        .unwrap()
                })
            };
            let ends = LoftEndPoints {
                start: point("start_point"),
                end: point("end_point"),
            };
            let style = match op["style"].as_str().unwrap() {
                "normal" => LoftStyle::Normal,
                "loose" => LoftStyle::Loose,
                "tight" => LoftStyle::Tight,
                "straight" => LoftStyle::Straight,
                "uniform" => LoftStyle::Uniform,
                _ => unreachable!(),
            };
            let actual = loft(
                &curves,
                style,
                op["closed"].as_bool().unwrap(),
                ends,
                op["rebuild_point_count"].as_u64().unwrap() as usize,
                Tolerance::DEFAULT,
            );
            assert_eq!(curves, before);
            let expected = row["value"]["surfaces"].as_array().unwrap();
            if expected.is_empty() {
                assert!(actual.is_err(), "{}: Native returned empty", op["id"]);
                continue;
            }
            let actual = match actual {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{}: {e:?}", op["id"]));
                    continue;
                }
            };
            let mut value = serde_json::json!({
                "degree":[actual.degree_u(),actual.degree_v()],
                "control_count":[actual.control_point_count_u(),actual.control_point_count_v()],
                "knots_u":actual.knots_u(),"knots_v":actual.knots_v(),
                "domain_u":[actual.domain_u().start(),actual.domain_u().end()],
                "domain_v":[actual.domain_v().start(),actual.domain_v().end()],
                "control_points":actual.control_points().iter().map(|p|serde_json::json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>()
            });
            let mut samples = Vec::new();
            for u in 0..=16 {
                for v in 0..=16 {
                    samples.push(
                        actual
                            .evaluate(
                                actual.parameter_at_u(f64::from(u) / 16.).unwrap(),
                                actual.parameter_at_v(f64::from(v) / 16.).unwrap(),
                            )
                            .unwrap()
                            .to_array(),
                    );
                }
            }
            value["samples"] = serde_json::json!(samples);
            match difference(&value, &expected[0]) {
                Ok(e) => {
                    maximum = maximum.max(e);
                    if e <= epsilon {
                        matched += 1;
                    } else {
                        failures.push(format!("{}: maximum error {e}", op["id"]));
                    }
                }
                Err(e) => failures.push(format!("{}: {e}", op["id"])),
            }
        }
    }
    println!(
        "LOFT_REBUILD matched={matched} max={maximum} failures={}",
        failures.len()
    );
    for f in &failures {
        println!("LOFT_REBUILD_DIFFERENCE {f}");
    }
    assert!(
        failures.is_empty(),
        "{} native Loft Rebuild differences",
        failures.len()
    );
}

pub fn difference(a: &Value, b: &Value) -> Result<f64, String> {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            if !a.is_finite() || !b.is_finite() {
                return Err("nonfinite value".into());
            }
            Ok((a - b).abs())
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .map(|(a, b)| difference(a, b))
            .try_fold(0_f64, |a, b| b.map(|b| a.max(b))),
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => a
            .iter()
            .map(|(k, v)| difference(v, &b[k]))
            .try_fold(0_f64, |a, b| b.map(|b| a.max(b))),
        _ if a == b => Ok(0.),
        _ => Err("surface definition layout differs".into()),
    }
}
