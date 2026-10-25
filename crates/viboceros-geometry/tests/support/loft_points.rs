//! Compare pole-honoring surfaces and retain native endpoint-omission diagnostics.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{LoftEndPoints, LoftStyle, NurbsCurve, NurbsSurface, Point3};
#[allow(dead_code)]
#[path = "loft_parameters.rs"]
mod definitions;

pub fn compare(actual: &Value, expected: &Value) -> Result<f64, String> {
    if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
        let error = (a - b).abs();
        return if a.is_finite() && error < 1e-9 {
            Ok(error)
        } else {
            Err(format!("{a} != {b} (error {error})"))
        };
    }
    match (actual, expected) {
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .map(|(a, b)| compare(a, b))
            .try_fold(0_f64, |a, b| b.map(|b| a.max(b))),
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => a
            .iter()
            .map(|(k, v)| compare(v, &b[k]).map_err(|e| format!("{k}: {e}")))
            .try_fold(0_f64, |a, b| b.map(|b| a.max(b))),
        _ if actual == expected => Ok(0.),
        _ => Err("surface definition layout differs".into()),
    }
}

pub fn honors_point_ends(surface: &Value, operation: &Value) -> bool {
    let nu = surface["control_count"][0].as_u64().unwrap() as usize;
    let nv = surface["control_count"][1].as_u64().unwrap() as usize;
    [("start_point", 0), ("end_point", nu - 1)]
        .into_iter()
        .all(|(key, u)| {
            let Some(point) = operation.get(key) else {
                return true;
            };
            (0..nv).all(|v| {
                (0..3).all(|axis| {
                    (surface["control_points"][v * nu + u]["point"][axis]
                        .as_f64()
                        .unwrap()
                        - point[axis].as_f64().unwrap())
                    .abs()
                        < 1e-9
                })
            })
        })
}

pub fn compare_end_points<E: std::fmt::Debug>(
    ignored_point_diagnostics: bool,
    loft: impl Fn(&[NurbsCurve], LoftStyle, LoftEndPoints) -> Result<NurbsSurface, E>,
) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let (mut matched, mut empty, mut extra, mut ignored_points, mut maximum) = (0, 0, 0, 0, 0_f64);
    for family in ["loft_points", "loft_points_more_sections"] {
        let q: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let operations = q["operations"].as_array().unwrap();
        let rows = r["results"].as_array().unwrap();
        assert_eq!(
            operations.len(),
            if family == "loft_points" { 90 } else { 60 }
        );
        assert_eq!(rows.len(), operations.len());
        for (op, row) in operations.iter().zip(rows) {
            assert_eq!(op["id"], row["id"]);
            assert_eq!(row["value"]["before"], row["value"]["after"]);
            let source = op["curves"]
                .as_array()
                .unwrap()
                .iter()
                .map(definitions::curve)
                .collect::<Vec<_>>();
            let before = source.clone();
            let style = match op["style"].as_str().unwrap() {
                "normal" => LoftStyle::Normal,
                "loose" => LoftStyle::Loose,
                "tight" => LoftStyle::Tight,
                "straight" => LoftStyle::Straight,
                "uniform" => LoftStyle::Uniform,
                _ => unreachable!(),
            };
            let point = |name: &str| {
                op.get(name).map(|v| {
                    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap())
                        .unwrap()
                })
            };
            let ends = LoftEndPoints {
                start: point("start_point"),
                end: point("end_point"),
            };
            let actual = loft(&source, style, ends);
            if let Ok(surface) = &actual {
                let nu = surface.control_point_count_u();
                for (u, point) in [(0, ends.start), (nu - 1, ends.end)] {
                    if let Some(point) = point {
                        assert!(
                            (0..surface.control_point_count_v())
                                .all(|v| surface.control_points()[v * nu + u].point() == point),
                            "{}: point end moved",
                            op["id"]
                        );
                    }
                }
            }
            assert_eq!(source, before);
            let native = row["value"]["surfaces"].as_array().unwrap();
            if native.is_empty() {
                if ignored_point_diagnostics {
                    continue;
                }
                empty += 1;
                if style == LoftStyle::Uniform {
                    assert!(
                        actual.is_err(),
                        "{}: native Uniform rejected point ends",
                        op["id"]
                    );
                } else if actual.is_ok() {
                    extra += 1;
                    println!("LOFT_POINTS_NATIVE_EMPTY {}", op["id"]);
                }
                continue;
            }
            assert_eq!(native.len(), 1);
            let ignores_points = !honors_point_ends(&native[0], op);
            if ignores_points {
                ignored_points += 1;
            }
            if ignored_point_diagnostics != ignores_points {
                continue;
            }
            let s = match actual {
                Ok(s) => s,
                Err(e) => {
                    failures.push(format!("{}: {e:?}", op["id"]));
                    continue;
                }
            };
            let mut samples = Vec::with_capacity(289);
            for u in 0..=16 {
                for v in 0..=16 {
                    samples.push(
                        s.evaluate(
                            s.parameter_at_u(u as f64 / 16.).unwrap(),
                            s.parameter_at_v(v as f64 / 16.).unwrap(),
                        )
                        .unwrap()
                        .to_array(),
                    );
                }
            }
            let actual = json!({"degree":[s.degree_u(),s.degree_v()],
                "control_count":[s.control_point_count_u(),s.control_point_count_v()],
                "knots_u":s.knots_u(),"knots_v":s.knots_v(),
                "domain_u":[s.domain_u().start(),s.domain_u().end()],
                "domain_v":[s.domain_v().start(),s.domain_v().end()],
                "control_points":s.control_points().iter().map(|p|json!({"point":p.point().to_array(),"weight":p.weight()})).collect::<Vec<_>>(),
                "samples":samples});
            match compare(&actual, &native[0]) {
                Ok(error) => {
                    maximum = maximum.max(error);
                    matched += 1;
                }
                Err(error) => failures.push(format!("{}: {error}", op["id"])),
            }
        }
    }
    println!(
        "LOFT_POINTS_SUMMARY {}",
        json!({"matched":matched,"native_empty":empty,"extra_line_results":extra,"native_ignored_point_ends":ignored_points,"maximum_error":maximum})
    );
    assert!(
        failures.is_empty(),
        "native point loft differences: {failures:?}"
    );
    if !ignored_point_diagnostics {
        assert_eq!((matched, empty, ignored_points, extra), (96, 50, 4, 20));
    }
}
