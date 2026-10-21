//! Corresponding quad bounds avoid ill-conditioned closest-point queries at a horn.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use viboceros_geometry::{Point3, SubD};
const EPSILON: f64 = 1e-6;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
fn center(points: [Point3; 4]) -> [f64; 3] {
    std::array::from_fn(|k| points.iter().map(|p| p.to_array()[k] * 0.25).sum())
}
fn bucket(p: [f64; 3]) -> [i64; 3] {
    p.map(|x| (x / EPSILON).floor() as i64)
}
fn warp(points: [Point3; 4]) -> f64 {
    let mut best = (0., [0.; 3], points[0]);
    for i in 0..4 {
        let a = points[i].vector_to(points[(i + 1) % 4]).unwrap();
        let b = points[i].vector_to(points[(i + 2) % 4]).unwrap();
        let normal = a.cross(b).unwrap();
        let norm = normal.length().unwrap();
        if norm > best.0 {
            best = (norm, normal.to_array(), points[i]);
        }
    }
    if best.0 == 0. {
        return 0.;
    }
    2. * points
        .iter()
        .map(|p| {
            let delta = best.2.vector_to(*p).unwrap().to_array();
            delta
                .iter()
                .zip(best.1)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                .abs()
                / best.0
        })
        .fold(0., f64::max)
}
pub fn without_surface(value: &Value, phase: &str) -> Value {
    let mut value = value.clone();
    for o in value[phase].as_array_mut().unwrap() {
        if let Some(s) = o.get_mut("subd").and_then(Value::as_object_mut) {
            s.remove("surface_mesh");
        }
    }
    value
}
pub fn certify(subd: &SubD, native: &Value, check: &mut Value) {
    let mesh = &native["surface_mesh"];
    let points = mesh["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect::<Vec<_>>();
    let faces = mesh["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let f = f.as_array().unwrap();
            assert_eq!(f.len(), 4);
            std::array::from_fn::<_, 4, _>(|i| points[f[i].as_u64().unwrap() as usize])
        })
        .collect::<Vec<_>>();
    let mut buckets: BTreeMap<[i64; 3], Vec<usize>> = BTreeMap::new();
    for (i, face) in faces.iter().enumerate() {
        buckets.entry(bucket(center(*face))).or_default().push(i);
    }
    let actual = subd.tessellate(2).unwrap();
    assert_eq!(actual.faces().len(), faces.len());
    let mut matched = vec![false; faces.len()];
    let mut maximum = 0_f64;
    for face in actual.faces() {
        let corners: [Point3; 4] = face
            .indices()
            .iter()
            .map(|&i| actual.vertices()[i as usize])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let key = bucket(center(corners));
        let mut best = (f64::INFINITY, usize::MAX);
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if let Some(candidates) = buckets.get(&[key[0] + x, key[1] + y, key[2] + z]) {
                        for &i in candidates {
                            if matched[i] {
                                continue;
                            }
                            for offset in 0..4 {
                                let error = (0..4)
                                    .map(|k| {
                                        corners[k].distance_to(faces[i][(k + offset) % 4]).unwrap()
                                    })
                                    .fold(0., f64::max);
                                if error < best.0 {
                                    best = (error, i);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(best.0 <= EPSILON, "unmatched torus facet, error {}", best.0);
        matched[best.1] = true;
        // Convex barycentric correspondence bounds each common triangulation.
        // Quad warp also bounds a change of diagonal in the regular torus grid.
        maximum = maximum.max(best.0 + warp(corners) + warp(faces[best.1]));
    }
    assert!(matched.iter().all(|&v| v));
    assert!(maximum <= EPSILON, "torus facet bound {maximum}");
    check["surface_sampled"] = json!(true);
    check["maximum_sampled_surface_error"] = json!(maximum);
    check["surface_facets_correspond"] = json!(true);
    check["compared_surface_faces"] = json!(faces.len());
}
