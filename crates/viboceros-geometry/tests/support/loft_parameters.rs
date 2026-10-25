//! Shared native section-chart checks for portable and usd-brep construction.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_geometry::{LoftStyle, NurbsCurve, NurbsSurface, Point3, WeightedPoint3};

pub fn curve(d: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        d["degree"].as_u64().unwrap() as usize,
        d["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                WeightedPoint3::try_new(
                    Point3::try_from(
                        serde_json::from_value::<[f64; 3]>(c["point"].clone()).unwrap(),
                    )
                    .unwrap(),
                    c["weight"].as_f64().unwrap(),
                )
                .unwrap()
            })
            .collect(),
        serde_json::from_value(d["knots"].clone()).unwrap(),
    )
    .unwrap()
}

const FLOATING_BOUNDARY_CASE: &str =
    "loft-domain-seam1-1-compatible-preparation-matching-affine-0-g2";

pub fn compare_preparation<E: std::fmt::Debug>(
    near_tie: bool,
    expected_count: usize,
    loft: impl Fn(&[NurbsCurve], LoftStyle, bool) -> Result<NurbsSurface, E>,
) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let mut cases = 0;
    for family in [
        "loft_splitting_basis_diagnostics",
        "loft_splitting_seam_diagnostics",
        "loft_splitting_domain_holdouts",
        "loft_splitting_domain_aspect_holdouts",
        "loft_splitting_domain_geometric_matching",
        "loft_splitting_domain_fastpath_holdouts",
        "loft_splitting_open_basis_holdouts",
    ] {
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
            if op["op"] != "loft" || op["command"] != false {
                continue;
            }
            if (op["id"].as_str().unwrap() == FLOATING_BOUNDARY_CASE) != near_tie {
                continue;
            }
            let curves = op["curves"]
                .as_array()
                .unwrap()
                .iter()
                .map(curve)
                .collect::<Vec<_>>();
            let before = curves.clone();
            let style = if op["style"] == "straight" {
                LoftStyle::Straight
            } else {
                LoftStyle::Normal
            };
            let actual = loft(&curves, style, false).unwrap();
            assert_eq!(curves, before);
            let native = if row["value"].is_array() {
                &row["value"][0]
            } else {
                &row["value"]["surfaces"][0]
            };
            let counts = [
                actual.control_point_count_u(),
                actual.control_point_count_v(),
            ];
            let mut error = 0_f64;
            for i in 0..=16 {
                for j in 0..=16 {
                    let point = actual
                        .evaluate(
                            actual.parameter_at_u(i as f64 / 16.).unwrap(),
                            actual.parameter_at_v(j as f64 / 16.).unwrap(),
                        )
                        .unwrap();
                    for (axis, value) in point.to_array().into_iter().enumerate() {
                        error = error.max(
                            (value - native["samples"][i * 17 + j][axis].as_f64().unwrap()).abs(),
                        );
                    }
                }
            }
            let mut coefficient_error = 0_f64;
            if actual.control_points().len() != native["control_points"].as_array().unwrap().len() {
                coefficient_error = f64::INFINITY;
            } else {
                for (a, b) in actual
                    .control_points()
                    .iter()
                    .zip(native["control_points"].as_array().unwrap())
                {
                    for (x, y) in a
                        .point()
                        .to_array()
                        .into_iter()
                        .zip(b["point"].as_array().unwrap())
                    {
                        coefficient_error = coefficient_error.max((x - y.as_f64().unwrap()).abs());
                    }
                    coefficient_error =
                        coefficient_error.max((a.weight() - b["weight"].as_f64().unwrap()).abs());
                }
            }
            let mut knot_error = 0_f64;
            for (knots, key) in [(actual.knots_u(), "knots_u"), (actual.knots_v(), "knots_v")] {
                if knots.len() != native[key].as_array().unwrap().len() {
                    knot_error = f64::INFINITY;
                    continue;
                }
                for (&a, b) in knots.iter().zip(native[key].as_array().unwrap()) {
                    knot_error = knot_error.max((a - b.as_f64().unwrap()).abs());
                }
            }
            println!(
                "LOFT_DOMAIN {}",
                json!({"id":op["id"],"counts":counts,"native_counts":native["control_count"],"sample_error":error,"knot_error":knot_error,"coefficient_error":coefficient_error})
            );
            if json!(counts) != native["control_count"]
                || error > 1e-9
                || knot_error > 1e-9
                || coefficient_error > 1e-9
            {
                failures.push(op["id"].clone());
            }
            cases += 1;
        }
    }
    assert_eq!(cases, expected_count);
    assert!(
        failures.is_empty(),
        "Loft domain comparisons failed: {failures:?}"
    );
}
