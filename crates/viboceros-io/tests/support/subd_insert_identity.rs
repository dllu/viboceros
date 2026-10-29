//! Native InsertPoint assigns fresh IDs in an order that varies between runs.
//! Resolve only new IDs by geometry; source component IDs stay authoritative.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use viboceros_geometry::SubD;

fn id(value: &Value) -> u32 {
    value.as_u64().unwrap() as u32
}
fn cyclic(value: &Value) -> Vec<u32> {
    let points = value.as_array().unwrap().iter().map(id).collect::<Vec<_>>();
    (0..points.len())
        .map(|offset| {
            (0..points.len())
                .map(|i| points[(i + offset) % points.len()])
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap()
}
pub(super) fn normalize(mut actual: Value, expected: &mut Value, source: &SubD) -> Value {
    let mut vertices = BTreeMap::new();
    for row in actual["vertices"].as_array().unwrap() {
        let source_id = id(&row[0]);
        let matches = expected["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|other| {
                if source.vertex_ids().contains(&source_id) {
                    return source_id == id(&other[0]);
                }
                !source.vertex_ids().contains(&id(&other[0]))
                    && row[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(other[1].as_array().unwrap())
                        .all(|(a, b)| (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-7)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "unique new vertex correspondence: {source_id}"
        );
        vertices.insert(source_id, id(&matches[0][0]));
    }
    for row in actual["vertices"].as_array_mut().unwrap() {
        row[0] = json!(vertices[&id(&row[0])]);
    }
    for row in actual["edges"].as_array_mut().unwrap() {
        let old = id(&row[0]);
        for vertex in row[1].as_array_mut().unwrap() {
            *vertex = json!(vertices[&id(vertex)]);
        }
        if !source.edge_ids().contains(&old) {
            let matches = expected["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|other| !source.edge_ids().contains(&id(&other[0])) && row[1] == other[1])
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "unique new edge correspondence: {old}");
            row[0] = matches[0][0].clone();
        }
    }
    for row in actual["faces"].as_array_mut().unwrap() {
        let old = id(&row[0]);
        for vertex in row[1].as_array_mut().unwrap() {
            *vertex = json!(vertices[&id(vertex)]);
        }
        let polygon = cyclic(&row[1]);
        if !source.face_ids().contains(&old) {
            let matches = expected["faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|other| {
                    !source.face_ids().contains(&id(&other[0])) && polygon == cyclic(&other[1])
                })
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "unique new face correspondence: {old}");
            row[0] = matches[0][0].clone();
        }
        row[1] = json!(polygon);
    }
    for row in expected["faces"].as_array_mut().unwrap() {
        row[1] = json!(cyclic(&row[1]));
    }
    for field in ["vertices", "edges", "faces"] {
        actual[field]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|row| id(&row[0]));
    }
    actual
}
