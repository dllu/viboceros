use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};

fn point(value: &Value) -> Point3 {
    Point3::try_new(
        value[0].as_f64().unwrap(),
        value[1].as_f64().unwrap(),
        value[2].as_f64().unwrap(),
    )
    .unwrap()
}
fn curve(value: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        value["degree"].as_u64().unwrap() as usize,
        value["control_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
            })
            .collect(),
        value["knots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect(),
    )
    .unwrap()
}

pub fn compare_boundaries(actual: &Value, full: &Value, path: &str) {
    // Quantization only selects a unique edge correspondence. Coordinates
    // and samples are then checked independently at the 1e-12 epsilon.
    fn key(row: &Value) -> (Vec<u64>, Vec<i64>) {
        let faces = row["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap())
            .collect();
        let quantize = |p: &Value| {
            p.as_array()
                .unwrap()
                .iter()
                .map(|v| (v.as_f64().unwrap() / 1e-6).round() as i64)
                .collect::<Vec<_>>()
        };
        let mut ends = row["ends"]
            .as_array()
            .unwrap()
            .iter()
            .map(quantize)
            .collect::<Vec<_>>();
        ends.sort();
        let mut spatial = ends.into_iter().flatten().collect::<Vec<_>>();
        spatial.extend(quantize(&row["points"][16]));
        (faces, spatial)
    }
    fn distance(a: &Value, b: &Value) -> f64 {
        a.as_array()
            .unwrap()
            .iter()
            .zip(b.as_array().unwrap())
            .map(|(x, y)| (x.as_f64().unwrap() - y.as_f64().unwrap()).powi(2))
            .sum::<f64>()
            .sqrt()
    }
    let actual = actual.as_array().unwrap();
    let full = full.as_array().unwrap();
    assert_eq!(actual.len(), full.len());
    for (a, b) in actual.iter().zip(full) {
        let mut uses = vec![Vec::new(); b["edges"].as_array().unwrap().len()];
        for (i, f) in b["topology"]["faces"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for l in f["loops"].as_array().unwrap() {
                for t in l["trims"].as_array().unwrap() {
                    if let Some(e) = t["edge"].as_u64() {
                        uses[e as usize].push(i);
                    }
                }
            }
        }
        let native = b["edges"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let ends = b["topology"]["edges"][i]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| b["vertices"][v.as_u64().unwrap() as usize]["point"].clone())
                    .collect::<Vec<_>>();
                let c = curve(&e["curve"]["definition"]);
                let points = (0..=32)
                    .map(|i| {
                        c.evaluate(c.parameter_at(i as f64 / 32.).unwrap())
                            .unwrap()
                            .to_array()
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({"faces":uses[i],"ends":ends,"points":points,"original_samples":e["curve"]["samples"]})
            })
            .collect::<Vec<_>>();
        let mut indexed = BTreeMap::new();
        for row in &native {
            assert!(
                indexed.insert(key(row), row).is_none(),
                "ambiguous native edge correspondence"
            );
        }
        assert_eq!(a.as_array().unwrap().len(), indexed.len());
        for row in a.as_array().unwrap() {
            let other = indexed
                .remove(&key(row))
                .expect("native edge with matching incidence and geometric contacts");
            let ends = row["ends"].as_array().unwrap();
            let native_ends = other["ends"].as_array().unwrap();
            assert!(
                ends.iter()
                    .zip(native_ends)
                    .all(|(a, b)| distance(a, b) < 1e-12)
                    || ends
                        .iter()
                        .zip(native_ends.iter().rev())
                        .all(|(a, b)| distance(a, b) < 1e-12),
                "{path}: native edge vertices differ by more than 1e-12"
            );
            let p = row["points"].as_array().unwrap();
            let q = other["points"].as_array().unwrap();
            assert_eq!(p.len(), q.len());
            let forward = p.iter().zip(q).all(|(a, b)| distance(a, b) < 1e-12);
            let reverse = p
                .iter()
                .zip(q.iter().rev())
                .all(|(a, b)| distance(a, b) < 1e-12);
            assert!(
                forward || reverse,
                "{path}: canonical NURBS edge correspondence exceeds 1e-12"
            );
            // Native analytic circle parameters are linear in angle; their
            // canonical rational NURBS parameters use a different clock.
            let raw = other["original_samples"].as_array().unwrap();
            if !p.iter().zip(raw).all(|(a, b)| distance(a, b) < 1e-12)
                && !p
                    .iter()
                    .zip(raw.iter().rev())
                    .all(|(a, b)| distance(a, b) < 1e-12)
            {
                let c = curve(&row["curve"]);
                let tolerance = Tolerance::try_new(1e-13, 1e-14, 1e-10).unwrap();
                for sample in raw {
                    let p = point(sample);
                    let t = c.closest_parameter(p, tolerance).unwrap();
                    assert!(
                        c.evaluate(t).unwrap().distance_to(p).unwrap() < 1e-12,
                        "{path}: native analytic edge sample leaves the candidate boundary"
                    );
                }
            }
        }
        assert!(indexed.is_empty());
    }
}
pub fn adjacency(full: &Value) -> Value {
    serde_json::json!(
        full.as_array()
            .unwrap()
            .iter()
            .map(|b| {
                let mut uses = vec![Vec::new(); b["edges"].as_array().unwrap().len()];
                for (i, f) in b["topology"]["faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                {
                    for l in f["loops"].as_array().unwrap() {
                        for t in l["trims"].as_array().unwrap() {
                            if let Some(e) = t["edge"].as_u64() {
                                uses[e as usize].push(i);
                            }
                        }
                    }
                }
                let mut pairs = uses
                    .into_iter()
                    .filter(|u| u.len() == 2)
                    .collect::<Vec<_>>();
                pairs.sort();
                pairs
            })
            .collect::<Vec<_>>()
    )
}
