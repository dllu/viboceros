//! Editing a surface can rebuild its boundary tables. Compare the complete
//! recorded geometry modulo table permutation and edge parameter orientation.
//! This never changes recorded coordinates or the comparison epsilon.
use serde_json::Value;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::Point3;
fn point(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(v.clone()).unwrap()).unwrap()
}
pub fn check(doc: &Document, expected: &Value, source: ObjectId, label: &str) -> f64 {
    let mut normalized = expected.clone();
    for row in normalized.as_array_mut().unwrap() {
        let original = row["id"].as_str().unwrap() == source.to_string();
        let object = if original {
            doc.object(source).unwrap()
        } else {
            doc.objects().find(|o| o.id() != source).unwrap()
        };
        let Geometry::Brep(b) = object.geometry() else {
            continue;
        };
        for (face, expected) in b
            .faces()
            .iter()
            .zip(row["geometry"]["surfaces"].as_array_mut().unwrap())
        {
            *expected = surface_definition(face.surface(), expected);
        }
        let topology = &mut row["geometry"]["topology"];
        let mut vertices = topology["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(Some)
            .collect::<Vec<_>>();
        assert_eq!(b.vertices().len(), vertices.len(), "{label}: vertex count");
        let mut aligned = Vec::new();
        for v in b.vertices() {
            let (i, d) = vertices
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    p.as_ref()
                        .map(|p| (i, v.point().distance_to(point(p)).unwrap()))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            assert!(d <= 1e-8, "{label}: unmatched vertex, distance {d}");
            aligned.push(vertices[i].take().unwrap());
        }
        topology["vertices"] = Value::Array(aligned);
        let mut edges = topology["edges"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(Some)
            .collect::<Vec<_>>();
        assert_eq!(b.edges().len(), edges.len(), "{label}: edge count");
        let mut aligned = Vec::new();
        for edge in b.edges() {
            let c = edge.curve();
            let domain = c.domain();
            let a = c.evaluate(*domain.start()).unwrap();
            let b = c.evaluate(*domain.end()).unwrap();
            let (i, d, reverse) = edges
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    e.as_ref().map(|e| {
                        let p = point(&e[0]);
                        let q = point(&e[1]);
                        let forward = a.distance_to(p).unwrap().max(b.distance_to(q).unwrap());
                        let backward = a.distance_to(q).unwrap().max(b.distance_to(p).unwrap());
                        if forward <= backward {
                            (i, forward, false)
                        } else {
                            (i, backward, true)
                        }
                    })
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            assert!(d <= 1e-8, "{label}: unmatched edge endpoints, distance {d}");
            let mut e = edges[i].take().unwrap();
            if reverse {
                e.as_array_mut().unwrap().reverse();
            }
            aligned.push(e);
        }
        topology["edges"] = Value::Array(aligned);
    }
    super::comparison::check(doc, &normalized, source, label)
}

fn surface_definition(actual: &viboceros_geometry::NurbsSurface, expected: &Value) -> Value {
    let mut definition = expected.clone();
    for (knots, key) in [(actual.knots_u(), "knots_u"), (actual.knots_v(), "knots_v")] {
        let values = definition[key].as_array_mut().unwrap();
        assert_eq!(values.len(), knots.len());
        // RhinoCommon omits these two unused outer slots. The producer pads
        // them by repetition; 3dm import may restore extrapolated values.
        values[0] = serde_json::json!(knots[0]);
        let last = values.len() - 1;
        values[last] = serde_json::json!(knots[last]);
    }
    definition
}
#[allow(dead_code)] // Used by standalone API replays; UI replays use check.
pub fn surface(actual: &viboceros_geometry::NurbsSurface, expected: &Value) -> f64 {
    super::comparison::surface(actual, &surface_definition(actual, expected))
}
