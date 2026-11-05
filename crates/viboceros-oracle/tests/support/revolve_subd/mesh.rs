//! Pair every oriented limit quad across different native display buffers.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
type Point = [f64; 3];
fn faces(mesh: &Value) -> Vec<Vec<Point>> {
    let vertices = mesh["vertices"].as_array().unwrap();
    let mut used = BTreeSet::new();
    let faces = mesh["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f.as_array()
                .unwrap()
                .iter()
                .map(|i| {
                    let i = i.as_u64().unwrap() as usize;
                    used.insert(i);
                    std::array::from_fn(|k| vertices[i][k].as_f64().unwrap())
                })
                .collect()
        })
        .collect();
    assert_eq!(used.len(), vertices.len(), "unreferenced limit vertices");
    faces
}
fn key(face: &[Point]) -> [i64; 3] {
    std::array::from_fn(|k| {
        ((face.iter().map(|p| p[k]).sum::<f64>() / face.len() as f64) / 1e-6).round() as i64
    })
}
fn align(actual: &Value, native: &Value) -> (Value, Value) {
    let a = faces(actual);
    let b = faces(native);
    assert_eq!(a.len(), b.len(), "native limit face count");
    let mut buckets = BTreeMap::<[i64; 3], Vec<usize>>::new();
    for (i, f) in b.iter().enumerate() {
        buckets.entry(key(f)).or_default().push(i);
    }
    let mut aligned = vec![None; b.len()];
    for mut f in a {
        let [x, y, z] = key(&f);
        let mut found = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &i in buckets.get(&[x + dx, y + dy, z + dz]).into_iter().flatten() {
                        if aligned[i].is_some() || f.len() != b[i].len() {
                            continue;
                        }
                        for shift in 0..f.len() {
                            if (0..f.len()).all(|j| {
                                (0..3).all(|k| {
                                    (f[(j + shift) % f.len()][k] - b[i][j][k]).abs() < 1e-6
                                })
                            }) {
                                assert!(
                                    found.replace((i, shift)).is_none(),
                                    "ambiguous oriented limit quad correspondence"
                                );
                            }
                        }
                    }
                }
            }
        }
        let (i, shift) = found.expect("corresponding native oriented limit quad");
        f.rotate_left(shift);
        aligned[i] = Some(f);
    }
    (
        serde_json::json!({"oriented_faces":aligned.into_iter().map(Option::unwrap).collect::<Vec<_>>()}),
        serde_json::json!({"oriented_faces":b}),
    )
}
pub(super) fn prepare(actual: &mut Value, native: &mut Value) {
    match (actual, native) {
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter_mut().zip(b) {
                prepare(a, b);
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            if let (Some(a), Some(b)) = (a.get_mut("surface_mesh"), b.get_mut("surface_mesh")) {
                let (x, y) = align(a, b);
                *a = x;
                *b = y;
            }
            for (k, a) in a.iter_mut().filter(|(k, _)| k.as_str() != "surface_mesh") {
                prepare(a, b.get_mut(k).unwrap());
            }
        }
        _ => {}
    }
}
