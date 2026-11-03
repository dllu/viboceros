//! Complete edge and oriented topology comparison against native records.
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_geometry::Brep;
#[path = "curve_full_image.rs"]
mod curves;
use curves::{curve, curve_bound};

fn cycle(values: &[usize]) -> Vec<usize> {
    (0..values.len())
        .map(|i| {
            values[i..]
                .iter()
                .chain(&values[..i])
                .copied()
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap()
}

pub fn check(actual: &Brep, value: &Value) {
    let target = &value["output"]["geometry"];
    let topology = &target["topology"];
    assert_eq!(
        actual.faces().len(),
        target["faces"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.edges().len(),
        target["edges"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.vertices().len(),
        target["vertices"].as_array().unwrap().len()
    );
    assert_eq!(
        actual.is_solid(),
        value["output"]["solid"].as_bool().unwrap()
    );
    assert!(actual.is_manifold());
    let mut vertex_map = Vec::new();
    let mut used = vec![false; actual.vertices().len()];
    for v in actual.vertices() {
        let (i, d) = target["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    i,
                    v.point()
                        .distance_to(crate::native::point(&p["point"]))
                        .unwrap(),
                )
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert!(d < 1e-11 && !used[i]);
        used[i] = true;
        vertex_map.push(i);
    }
    let mut expected_edges = BTreeMap::new();
    for (i, edge) in topology["edges"].as_array().unwrap().iter().enumerate() {
        let (a, b) = (
            edge[0].as_u64().unwrap() as usize,
            edge[1].as_u64().unwrap() as usize,
        );
        assert!(expected_edges.insert([a.min(b), a.max(b)], i).is_none());
    }
    let mut maximum = 0_f64;
    for edge in actual.edges() {
        let [a, b] = edge.vertices().map(|i| vertex_map[i]);
        let i = expected_edges[&[a.min(b), a.max(b)]];
        let mut expected = curve(&target["edges"][i]["curve"]["definition"]);
        if topology["edges"][i][0].as_u64().unwrap() as usize != a {
            expected = expected.reversed().unwrap();
        }
        maximum = maximum.max(curve_bound(edge.curve(), &expected));
    }
    assert!(maximum < 1e-11, "edge full-image bound {maximum}");
    for (face_index, face) in actual.faces().iter().enumerate() {
        assert_eq!(face.loops().len(), 1);
        let boundary = &face.loops()[0];
        let expected = &topology["faces"][face_index]["loops"][0]["trims"];
        let expected = expected.as_array().unwrap();
        assert_eq!(boundary.trims().len(), expected.len());
        let a = boundary
            .trims()
            .iter()
            .map(|t| vertex_map[t.vertices()[0]])
            .collect::<Vec<_>>();
        let b = expected
            .iter()
            .map(|t| t["vertices"][0].as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        assert_eq!(cycle(&a), cycle(&b));
        for trim in boundary.trims() {
            let first = vertex_map[trim.vertices()[0]];
            let native = expected
                .iter()
                .find(|t| t["vertices"][0].as_u64().unwrap() as usize == first)
                .unwrap();
            assert_eq!(
                format!("{:?}", trim.trim_type()),
                native["type"].as_str().unwrap()
            );
        }
    }
}
