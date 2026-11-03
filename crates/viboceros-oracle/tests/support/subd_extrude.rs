pub use super::source_archives::sources;
use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{SubD, SubDExtrudeSelection as Selection};
pub fn selection(target: &str) -> Selection {
    if target.ends_with("whole") {
        return Selection::AllFaces;
    }
    let (faces, edges) = match target {
        "bent_redundant" | "creased_redundant" => (vec![0], vec![0, 1]),
        "bent_separate" => (vec![0], vec![6]),
        "grid_mixed" => (vec![1], vec![0, 1]),
        "quad_loop" => (vec![], vec![0, 1, 2, 3]),
        "bent_edges" | "grid_edges" => (vec![], vec![0, 1]),
        t if t.ends_with("edge") => (vec![], vec![0]),
        "grid_face" | "creased_other" => (vec![1], vec![]),
        "grid_disjoint" => (vec![0, 5], vec![]),
        "closed_faces" => (vec![0, 2], vec![]),
        t if t.ends_with("faces") => (vec![0, 1], vec![]),
        _ => (vec![0], vec![]),
    };
    Selection::Components { faces, edges }
}

pub fn fixture(source: &SubD) -> (Document, ObjectId) {
    let mut doc = Document::default();
    let layer = doc
        .add_layer("SubD extrude source", ColorRgb::new(0, 0, 255))
        .unwrap();
    let id = doc
        .add_geometry_with_attributes(
            Geometry::SubD(source.clone()),
            ObjectAttributes::on_layer(layer)
                .with_name("Input")
                .with_object_color(ColorRgb::new(17, 43, 91)),
        )
        .unwrap();
    doc.add_group(Some("SubD extrude group".into()), [id])
        .unwrap();
    doc.clear_history().unwrap();
    (doc, id)
}
pub fn expected(v: &Value) -> Value {
    let mut value = viboceros_oracle::subd_join::expected(v);
    for row in value.as_array_mut().unwrap() {
        let row = row.as_object_mut().unwrap();
        row.remove("selected_faces");
        row.remove("selected_edges");
    }
    value
}
pub fn options(op: &Value) -> viboceros_command::extrude_subd::Options {
    use viboceros_command::extrude_subd::{Basis, Direction, Options};
    let (basis, direction) = match op["mode"].as_str().unwrap() {
        "Z" => (Basis::Wcs, Direction::Z),
        "V" => (Basis::Uvn, Direction::V),
        _ => (Basis::Uvn, Direction::N),
    };
    Options {
        distance: op["distance"].as_f64().unwrap(),
        basis,
        direction,
    }
}
