//! Frozen source archives and explicit native getter traces for face insets.
use super::common;
use serde_json::{Value, json};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{SubD, TriangleMesh};
pub fn source(native: &Value) -> Geometry {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.3dm");
    let bytes = native["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let (doc, _, _, _, _) =
        viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
    doc.objects().next().unwrap().geometry().clone()
}
pub fn fixture(native: &Value) -> (Document, ObjectId) {
    let mut doc = Document::default();
    let layer = doc
        .add_layer("Inset source", ColorRgb::new(0, 0, 255))
        .unwrap();
    let id = doc
        .add_geometry_with_attributes(
            source(native),
            ObjectAttributes::on_layer(layer)
                .with_name("Input")
                .with_object_color(ColorRgb::new(17, 43, 91)),
        )
        .unwrap();
    doc.add_group(Some("Inset group".into()), [id]).unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
pub fn indices(op: &Value) -> Option<Vec<usize>> {
    Some(match op["target"].as_str().unwrap() {
        t if t.ends_with("whole") => return None,
        "bent_both" | "creased_both" | "grid_pair" | "triangles_both" | "disjoint" => vec![0, 1],
        "grid_face" => vec![1],
        "grid_concave" => vec![0, 1, 3],
        "grid_vertex" => vec![0, 4],
        "closed_both" => vec![0, 2],
        "closed_all" | "grid_all" => (0..6).collect(),
        "ring" => (0..8).collect(),
        _ => vec![0],
    })
}
pub fn effective_distance(op: &Value, native: &Value, family: &str) -> f64 {
    if family == "inset_mesh_ready" {
        return op["distance"].as_f64().unwrap();
    }
    let requested = op["distance"].as_f64().unwrap();
    if family.starts_with("inset_mesh") || requested <= 0. {
        // These saved macros show Rhino's previous mesh preview distance, or
        // its reprompt followed by Enter for a nonpositive SubD distance.
        native["history"]
            .as_str()
            .unwrap()
            .split_once('<')
            .unwrap()
            .1
            .split_once('>')
            .unwrap()
            .0
            .parse()
            .unwrap()
    } else {
        requested
    }
}
pub fn options(op: &Value, distance: f64) -> viboceros_command::inset::Options {
    viboceros_command::inset::Options {
        distance,
        mode: if op["mode"] == "Group" {
            viboceros_command::inset::Mode::Group
        } else {
            viboceros_command::inset::Mode::Single
        },
        ..Default::default()
    }
}
pub fn subd_value(s: &SubD) -> Value {
    json!({"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"surface_points":s.surface_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":s.faces(),"edges":s.edges(),"vertex_tags":s.vertex_tags(),"edge_tags":s.edge_tags(),"valid":true})
}
pub fn mesh_value(m: &TriangleMesh) -> Value {
    json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices()).collect::<Vec<_>>(),"normals":m.vertex_normals().map(|v|v.iter().map(|p|p.to_array()).collect::<Vec<_>>()).unwrap_or_default(),"uv":m.texture_coordinates().unwrap_or_default(),"colors":m.vertex_colors().map(|v|v.iter().map(|&[r,g,b,a]|[r,g,b,255-a]).collect::<Vec<_>>()).unwrap_or_default(),"ngons":m.ngons().iter().map(|g|json!({"vertices":g.vertices(),"faces":g.faces()})).collect::<Vec<_>>()})
}
pub fn expected_subd(value: &Value) -> Value {
    let mut v = value.clone();
    v.as_object_mut().unwrap().remove("surface_mesh");
    v
}
pub fn expected_mesh(value: &Value, changed: bool) -> Value {
    let mut v = value.clone();
    let count = v["vertices"].as_array().unwrap().len();
    if v["colors"].as_array().unwrap().len() != count && !v["colors"].as_array().unwrap().is_empty()
    {
        // Rhino retains a raw prefix that its HasVertexColors predicate rejects.
        // The Rust model represents only complete channels.
        if let Some(has) = v.get("has_colors") {
            assert_eq!(has, false);
        }
        v["colors"] = json!([]);
    }
    if changed && !v["ngons"].as_array().unwrap().is_empty() {
        // The native command leaves stale n-gon boundaries around edited raw
        // polygons. Our mesh invariant drops those stale logical records.
        v["ngons"] = json!([]);
    }
    v.as_object_mut().unwrap().remove("valid");
    v.as_object_mut().unwrap().remove("has_colors");
    v
}
pub fn snapshot(doc: &Document, id: ObjectId) -> Value {
    assert_eq!(doc.objects().len(), 1, "owned single-source inset fixture");
    let object = doc.object(id).unwrap();
    let a = object.attributes();
    let color = a.object_color();
    let mut row = json!({"source":0,"selected":doc.is_selected(id),"name":a.name(),"layer":doc.layer(a.layer_id()).unwrap().name(),"color":[color.red,color.green,color.blue],"color_source":a.color_source()as u8,"groups":object.group_ids().iter().map(|&id|doc.group(id).unwrap().name()).collect::<Vec<_>>()});
    match object.geometry() {
        Geometry::SubD(s) => row["subd"] = subd_value(s),
        Geometry::Mesh(m) => row["mesh"] = mesh_value(m),
        _ => panic!(),
    };
    json!([row])
}
pub fn expected_rows(rows: &Value, changed: bool) -> Value {
    let mut rows = rows.clone();
    for row in rows.as_array_mut().unwrap() {
        row.as_object_mut().unwrap().remove("selected_faces");
        if !row["subd"].is_null() {
            row["subd"] = expected_subd(&row["subd"]);
        } else {
            row["mesh"] = expected_mesh(&row["mesh"], changed);
        }
    }
    rows
}
pub fn compare(a: &Value, b: &Value, label: &str) {
    common::compare(a, b, label)
}
