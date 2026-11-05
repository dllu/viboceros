//! Preserve every used B-rep definition after component correspondence.
use super::{cap_charts, subd_apex_comparison};

use serde_json::Value;
use std::collections::BTreeMap;
pub fn compare(actual: &Value, native: &Value, path: &str, epsilon: f64) {
    let mut actual = actual.clone();
    let mut native = native.clone();
    used_surfaces(&mut native);
    padding(&mut actual);
    padding(&mut native);

    cap_charts::align(&mut actual, &native, true);
    align_vertices(&mut actual, &native);
    subd_apex_comparison::compare(&actual, &native, path, epsilon);
}
// OpenNURBS/3DM omits the two unused full-vector padding entries. Their
// reconstruction is storage policy; every active/supporting knot stays intact.
fn padding(v: &mut Value) {
    match v {
        Value::Array(a) => a.iter_mut().for_each(padding),
        Value::Object(a) => {
            for key in ["knots", "knots_u", "knots_v"] {
                if let Some(knots) = a.get_mut(key).and_then(Value::as_array_mut) {
                    let last = knots.len() - 1;
                    knots[0] = knots[1].clone();
                    knots[last] = knots[last - 1].clone();
                }
            }
            a.values_mut().for_each(padding);
        }
        _ => {}
    }
}
fn used_surfaces(v: &mut Value) {
    match v {
        Value::Array(a) => a.iter_mut().for_each(used_surfaces),
        Value::Object(a) => {
            if a.contains_key("brep") && a.contains_key("surfaces") {
                a["surfaces"] = serde_json::json!(
                    a["brep"]["faces"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| f["definition"].clone())
                        .collect::<Vec<_>>()
                );
            } else {
                a.values_mut().for_each(used_surfaces);
            }
        }
        _ => {}
    }
}
fn align_vertices(actual: &mut Value, native: &Value) {
    match (actual, native) {
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter_mut().zip(b) {
                align_vertices(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.contains_key("topology") && a.contains_key("vertices") && a.contains_key("edges") {
                let key = |p: &Value| {
                    p["point"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| (v.as_f64().unwrap() / 1e-6).round() as i64)
                        .collect::<Vec<_>>()
                };
                let mut lookup = BTreeMap::new();
                for (i, v) in b["vertices"].as_array().unwrap().iter().enumerate() {
                    assert!(lookup.insert(key(v), i).is_none());
                }
                let permutation = a["vertices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| lookup.remove(&key(v)).expect("native vertex"))
                    .collect::<Vec<_>>();
                assert!(lookup.is_empty());
                let mut inverse = vec![0; permutation.len()];
                for (i, &p) in permutation.iter().enumerate() {
                    inverse[p] = i;
                }
                a["vertices"] = serde_json::json!(
                    inverse
                        .iter()
                        .map(|&i| a["vertices"][i].clone())
                        .collect::<Vec<_>>()
                );
                for edge in a["topology"]["edges"].as_array_mut().unwrap() {
                    for v in edge.as_array_mut().unwrap() {
                        *v = serde_json::json!(permutation[v.as_u64().unwrap() as usize]);
                    }
                }
                for face in a["topology"]["faces"].as_array_mut().unwrap() {
                    for boundary in face["loops"].as_array_mut().unwrap() {
                        for trim in boundary["trims"].as_array_mut().unwrap() {
                            for v in trim["vertices"].as_array_mut().unwrap() {
                                *v = serde_json::json!(permutation[v.as_u64().unwrap() as usize]);
                            }
                        }
                    }
                }
            } else {
                for (k, v) in a {
                    align_vertices(v, &b[k]);
                }
            }
        }
        _ => {}
    }
}
