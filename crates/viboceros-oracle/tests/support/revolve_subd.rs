use serde_json::Value;
#[path = "revolve_subd/mesh.rs"]
mod mesh;
#[path = "revolve_subd/values.rs"]
mod values;

pub fn epsilon(id: &str) -> f64 {
    // Native fitting near a 3e6 world origin differs by up to 1.2e-9.
    // Every other fixed case retains the strict absolute 1e-12 threshold.
    if id == "line_translated" { 2e-9 } else { 1e-12 }
}

pub fn compare(actual: &Value, native: &Value, path: &str, epsilon: f64) {
    let mut actual = align_vertices(actual, native);
    let mut native = native.clone();
    mesh::prepare(&mut actual, &mut native);
    values::compare(
        &canonical_edges(&actual),
        &canonical_edges(&native),
        path,
        epsilon,
    );
}

/// Independent builders may insert the same tagged edge in a different order.
/// Retain oriented face cycles and every geometric sample after correspondence.
pub fn canonical_edges(value: &Value) -> Value {
    match value {
        Value::Array(a) => Value::Array(a.iter().map(canonical_edges).collect()),
        Value::Object(a) => {
            let mut a = a
                .iter()
                .map(|(k, v)| (k.clone(), canonical_edges(v)))
                .collect::<serde_json::Map<_, _>>();
            if a.contains_key("edges") && a.contains_key("edge_tags") {
                let faces = a["faces"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| {
                        let mut f = f.as_array().unwrap().clone();
                        let first = (0..f.len())
                            .min_by_key(|&i| f[i].as_u64().unwrap())
                            .unwrap();
                        f.rotate_left(first);
                        f
                    })
                    .collect::<Vec<_>>();
                a["faces"] = serde_json::json!(faces);
                let mut pairs = a["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(a["edge_tags"].as_array().unwrap())
                    .map(|(p, tag)| {
                        let mut p = p
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|i| i.as_u64().unwrap())
                            .collect::<Vec<_>>();
                        p.sort_unstable();
                        (p, tag.clone())
                    })
                    .collect::<Vec<_>>();
                pairs.sort_by(|a, b| a.0.cmp(&b.0));
                a["edges"] = serde_json::json!(pairs.iter().map(|p| &p.0).collect::<Vec<_>>());
                a["edge_tags"] = serde_json::json!(pairs.iter().map(|p| &p.1).collect::<Vec<_>>());
            }
            Value::Object(a)
        }
        x => x.clone(),
    }
}

/// Match unique geometric control vertices before comparing tagged incidence.
/// The correspondence grid is not the comparison epsilon.
pub fn align_vertices(actual: &Value, native: &Value) -> Value {
    use std::collections::BTreeMap;
    match (actual, native) {
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "native array length");
            Value::Array(a.iter().zip(b).map(|(a, b)| align_vertices(a, b)).collect())
        }
        (Value::Object(a), Value::Object(b)) => {
            let mut a = a
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        if k == "surface_mesh" {
                            v.clone()
                        } else {
                            align_vertices(v, &b[k])
                        },
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            if a.contains_key("vertex_tags") && a.contains_key("surface_points") {
                let key = |p: &Value| {
                    p.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| (v.as_f64().unwrap() / 1e-6).round() as i64)
                        .collect::<Vec<_>>()
                };
                let mut index = BTreeMap::new();
                for (i, p) in b["points"].as_array().unwrap().iter().enumerate() {
                    assert!(
                        index.insert(key(p), i).is_none(),
                        "ambiguous native control vertex"
                    );
                }
                let permutation = a["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        index
                            .remove(&key(p))
                            .expect("corresponding native control vertex")
                    })
                    .collect::<Vec<_>>();
                assert!(index.is_empty());
                let mut inverse = vec![0; permutation.len()];
                for (i, &p) in permutation.iter().enumerate() {
                    inverse[p] = i;
                }
                for name in ["points", "surface_points", "vertex_tags"] {
                    a[name] = serde_json::json!(
                        inverse
                            .iter()
                            .map(|&i| a[name][i].clone())
                            .collect::<Vec<_>>()
                    );
                }
                for name in ["edges", "faces"] {
                    a[name] = serde_json::json!(
                        a[name]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|f| f
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|i| permutation[i.as_u64().unwrap() as usize])
                                .collect::<Vec<_>>())
                            .collect::<Vec<_>>()
                    );
                }
            }
            Value::Object(a)
        }
        _ => actual.clone(),
    }
}
