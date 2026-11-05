//! Full fallback B-rep definitions with native edge-slot correspondence.
use super::subd_face_comparison;
use serde_json::Value;
use std::collections::BTreeMap;

pub fn compare(actual: &Value, native: &Value, path: &str, epsilon: f64) {
    let mut actual = actual.clone();
    align_edges(&mut actual, native);
    subd_face_comparison::compare(&actual, native, path, epsilon);
}
fn align_edges(actual: &mut Value, native: &Value) {
    match (actual, native) {
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter_mut().zip(b) {
                align_edges(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            if a.contains_key("topology") && a.contains_key("edges") && a.contains_key("vertices") {
                let key = |edge: &Value| {
                    let controls = edge["curve"]["definition"]["control_points"]
                        .as_array()
                        .unwrap();
                    [controls.first().unwrap(), controls.last().unwrap()]
                        .into_iter()
                        .flat_map(|p| p["point"].as_array().unwrap())
                        .map(|v| (v.as_f64().unwrap() / 1e-6).round() as i64)
                        .collect::<Vec<_>>()
                };
                let mut lookup = BTreeMap::new();
                for (i, edge) in b["edges"].as_array().unwrap().iter().enumerate() {
                    assert!(
                        lookup.insert(key(edge), i).is_none(),
                        "ambiguous native oriented edge"
                    );
                }
                let permutation = a["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|edge| lookup.remove(&key(edge)).expect("native oriented edge"))
                    .collect::<Vec<_>>();
                assert!(lookup.is_empty());
                let mut inverse = vec![0; permutation.len()];
                for (i, &p) in permutation.iter().enumerate() {
                    inverse[p] = i;
                }
                a["edges"] = serde_json::json!(
                    inverse
                        .iter()
                        .map(|&i| a["edges"][i].clone())
                        .collect::<Vec<_>>()
                );
                a["topology"]["edges"] = serde_json::json!(
                    inverse
                        .iter()
                        .map(|&i| a["topology"]["edges"][i].clone())
                        .collect::<Vec<_>>()
                );
                for face in a["topology"]["faces"].as_array_mut().unwrap() {
                    for boundary in face["loops"].as_array_mut().unwrap() {
                        for trim in boundary["trims"].as_array_mut().unwrap() {
                            if let Some(i) = trim["edge"].as_u64() {
                                trim["edge"] = serde_json::json!(permutation[i as usize]);
                            }
                        }
                    }
                }
                // Every curve, knot, weight, tolerance, endpoint, UV trim and
                // oriented incidence is still compared after this permutation.
            } else {
                for (key, value) in a {
                    if key != "surface_mesh" {
                        align_edges(value, &b[key]);
                    }
                }
            }
        }
        _ => {}
    }
}
