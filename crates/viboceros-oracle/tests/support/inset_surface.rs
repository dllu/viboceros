//! Shared spatial and incidence comparisons for planar inset evidence.
use super::common;
use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{Brep, LineSegment, Point3, Tolerance};

pub fn source(record: &Value) -> Brep {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("surface.3dm");
    let bytes = record["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let (doc, _, _, _, _) =
        viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
    let Geometry::Brep(brep) = doc.objects().next().unwrap().geometry() else {
        panic!("expected B-rep")
    };
    brep.clone()
}
pub fn fixture(record: &Value) -> (Document, ObjectId) {
    let mut doc = Document::default();
    doc.set_tolerance(Tolerance::try_new(1e-6, 1e-12, 1e-6).unwrap());
    let layer = doc
        .add_layer("Inset surface source", ColorRgb::new(0, 0, 255))
        .unwrap();
    let id = doc
        .add_geometry_with_attributes(
            Geometry::Brep(source(record)),
            ObjectAttributes::on_layer(layer)
                .with_name("Input")
                .with_object_color(ColorRgb::new(17, 43, 91)),
        )
        .unwrap();
    doc.add_group(Some("Inset surface group".into()), [id])
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
pub fn assert_metadata(doc: &Document, id: ObjectId, native: &Value) {
    assert_eq!(doc.objects().len(), 1);
    let obj = doc.object(id).unwrap();
    let a = obj.attributes();
    let c = a.object_color();
    let actual = serde_json::json!({"source":0,"selected":doc.is_selected(id),
        "name":a.name(),"layer":doc.layer(a.layer_id()).map(|l| l.name()),
        "color":[c.red,c.green,c.blue],
        "groups":obj.group_ids().iter().map(|&g| doc.group(g).unwrap().name()).collect::<Vec<_>>()});
    let expected = serde_json::json!({"source":native["source"],"selected":native["selected"],
        "name":native["name"],"layer":if native["layer"] == 0 { Some("Inset surface source") } else { None },
        "color":native["color"],"groups":native["groups"].as_array().unwrap().iter().map(|g| {
            assert_eq!(g,0); Some("Inset surface group")
        }).collect::<Vec<_>>()});
    common::compare(&actual, &expected, "surface inset metadata");
}
fn point(v: &Value) -> Point3 {
    Point3::try_new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
    .unwrap()
}
fn cycle(mut vertices: Vec<usize>) -> Vec<usize> {
    let pivot = vertices
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| *v)
        .unwrap()
        .0;
    vertices.rotate_left(pivot);
    vertices
}
fn topology(brep: &Brep, mapping: &[usize]) -> Value {
    let mut edges = brep
        .edges()
        .iter()
        .map(|e| {
            let mut ends = e.vertices().map(|v| mapping[v]);
            ends.sort();
            ends
        })
        .collect::<Vec<_>>();
    edges.sort();
    let mut faces = brep
        .faces()
        .iter()
        .map(|f| {
            let mut loops = f
                .loops()
                .iter()
                .map(|l| cycle(l.trims().iter().map(|t| mapping[t.vertices()[0]]).collect()))
                .collect::<Vec<_>>();
            loops[1..].sort();
            (f.is_reversed(), loops)
        })
        .collect::<Vec<_>>();
    faces.sort();
    serde_json::json!({"edges": edges, "faces": faces, "solid":brep.is_solid()})
}
fn native_topology(record: &Value) -> Value {
    let mut edges = record["topology"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let mut ends = [
                e[0].as_u64().unwrap() as usize,
                e[1].as_u64().unwrap() as usize,
            ];
            ends.sort();
            ends
        })
        .collect::<Vec<_>>();
    edges.sort();
    let mut faces = record["topology"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let mut loops = f["loops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| {
                    cycle(
                        l["trims"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|t| t["vertices"][0].as_u64().unwrap() as usize)
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            loops[1..].sort();
            (f["reversed"].as_bool().unwrap(), loops)
        })
        .collect::<Vec<_>>();
    faces.sort();
    serde_json::json!({"edges":edges,"faces":faces,"solid":record["topology"]["solid"]})
}

/// Clocks and raw table order differ; match vertices bijectively, then compare
/// every oriented face loop and every edge endpoint pair. Spatial station
/// checks use those corresponding edges, never nearest unrelated geometry.
pub fn assert_geometry(output: &Brep, expected: &Value, tolerance: Tolerance, id: &str) {
    let vertices = expected["vertices"].as_array().unwrap();
    assert_eq!(output.vertices().len(), vertices.len(), "{id}");
    let mapping = output
        .vertices()
        .iter()
        .map(|v| {
            vertices
                .iter()
                .position(|w| v.point().distance_to(point(&w["point"])).unwrap() <= 1e-10)
                .unwrap_or_else(|| panic!("{id}: unmatched vertex {:?}", v.point()))
        })
        .collect::<Vec<_>>();
    let mut distinct = mapping.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        mapping.len(),
        "{id}: bijective vertex correspondence"
    );
    common::compare(&topology(output, &mapping), &native_topology(expected), id);
    for (i, edge) in expected["edges"].as_array().unwrap().iter().enumerate() {
        let ends = &expected["topology"]["edges"][i];
        let ends = [
            ends[0].as_u64().unwrap() as usize,
            ends[1].as_u64().unwrap() as usize,
        ];
        let actual = output
            .edges()
            .iter()
            .find(|e| {
                let mapped = e.vertices().map(|v| mapping[v]);
                mapped == ends || mapped == [ends[1], ends[0]]
            })
            .unwrap();
        assert_eq!(actual.curve().degree(), 1);
        assert_eq!(actual.curve().control_points().len(), 2);
        let ends = actual.vertices().map(|v| output.vertices()[v].point());
        let line = LineSegment::try_new(ends[0], ends[1], tolerance).unwrap();
        for p in edge["curve"]["samples"].as_array().unwrap() {
            let p = point(p);
            let distance = line
                .closest_point(p, tolerance)
                .unwrap()
                .distance_to(p)
                .unwrap();
            assert!(distance <= 1e-10, "{id}: edge witness {distance}");
        }
    }
}
