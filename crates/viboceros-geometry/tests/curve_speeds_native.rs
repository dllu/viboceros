//! Native speed queries checked against captured public Rhino derivatives.
use serde_json::Value;
use std::path::Path;
use viboceros_geometry::{NurbsCurve, Point3, WeightedPoint3};

fn curve(d: &Value) -> NurbsCurve {
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

#[test]
fn native_speed_rounding_matches_licensed_queries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let family = "loft_splitting_domain_speed_jets";
    let q: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
    )
    .unwrap();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        for (definition, native) in op["curves"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["value"]["curve_speeds"].as_array().unwrap())
        {
            let source = curve(definition);
            let before = source.clone();
            let controls = source
                .control_points()
                .iter()
                .map(|p| {
                    let q = p.point();
                    [q.x(), q.y(), q.z(), p.weight()]
                })
                .collect::<Vec<_>>();
            let parameters: Vec<f64> =
                serde_json::from_value(native["parameters"].clone()).unwrap();
            let actual = viboceros_opennurbs::curve_speeds(
                source.degree(),
                &controls,
                source.knots(),
                &parameters,
            )
            .unwrap();
            for (i, (a, b)) in actual
                .into_iter()
                .zip(native["speeds"].as_array().unwrap())
                .enumerate()
            {
                let _managed_speed = b.as_f64().unwrap();
                let b = opennurbs_length(
                    serde_json::from_value(native["derivatives"][i].clone()).unwrap(),
                );
                assert_eq!(
                    a.to_bits(),
                    b.to_bits(),
                    "{} sample {i}: {a:?} != {b:?}",
                    op["id"]
                );
                count += 1;
            }
            assert_eq!(source, before);
        }
    }
    assert_eq!(count, 264);
}

// RhinoCommon Vector3d.Length multiplies by a reciprocal. Public OpenNURBS
// ON_Length3d divides; retain that difference when checking captured derivatives.
fn opennurbs_length([x, y, z]: [f64; 3]) -> f64 {
    let (mut x, mut y, mut z) = (x.abs(), y.abs(), z.abs());
    if y >= x && y >= z {
        std::mem::swap(&mut x, &mut y);
    } else if z >= x && z >= y {
        std::mem::swap(&mut x, &mut z);
    }
    if x > f64::MIN_POSITIVE {
        y /= x;
        z /= x;
        x * (1. + y * y + z * z).sqrt()
    } else {
        x
    }
}

#[test]
fn native_span_speeds_match_licensed_quadrature_point_queries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let family = "loft_splitting_domain_quadrature";
    let q: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
    )
    .unwrap();
    let r: Value = serde_json::from_slice(
        &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
    )
    .unwrap();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        for (definition, spans) in op["curves"]
            .as_array()
            .unwrap()
            .iter()
            .zip(row["value"]["quadrature"].as_array().unwrap())
        {
            let source = curve(definition);
            let before = source.clone();
            for span in spans.as_array().unwrap() {
                let domain = &span["source_domain"];
                let normalized = source
                    .try_trimmed(domain[0].as_f64().unwrap()..=domain[1].as_f64().unwrap())
                    .unwrap()
                    .try_reparameterized(0. ..=1.)
                    .unwrap();
                let controls = normalized
                    .control_points()
                    .iter()
                    .map(|p| {
                        let q = p.point();
                        [q.x(), q.y(), q.z(), p.weight()]
                    })
                    .collect::<Vec<_>>();
                let samples = span["samples"].as_array().unwrap();
                let parameters = samples
                    .iter()
                    .map(|s| s["parameter"].as_f64().unwrap())
                    .collect::<Vec<_>>();
                let actual = viboceros_opennurbs::curve_speeds(
                    normalized.degree(),
                    &controls,
                    normalized.knots(),
                    &parameters,
                )
                .unwrap();
                for (i, (a, b)) in actual.into_iter().zip(samples).enumerate() {
                    let b =
                        opennurbs_length(serde_json::from_value(b["derivative"].clone()).unwrap());
                    assert_eq!(
                        a.to_bits(),
                        b.to_bits(),
                        "{} {domain} sample {i}: {a:?} != {b:?}",
                        op["id"]
                    );
                    count += 1;
                }
            }
            assert_eq!(source, before);
        }
    }
    assert_eq!(count, 1056);
}
