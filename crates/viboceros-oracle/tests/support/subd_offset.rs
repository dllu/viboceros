pub use super::common::{cases_for, compare};
pub use super::source_archives::sources;
use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::SubD;
pub fn fixture(source: &SubD) -> (Document, ObjectId) {
    let (doc, ids) = fixture_many(&[source]);
    (doc, ids[0])
}
pub fn fixture_many(sources: &[&SubD]) -> (Document, Vec<ObjectId>) {
    let mut doc = Document::default();
    let layer = doc
        .add_layer("SubD offset source", ColorRgb::new(0, 0, 255))
        .unwrap();
    let current = doc
        .add_layer("SubD offset current", ColorRgb::new(0, 0, 255))
        .unwrap();
    doc.set_current_layer(current).unwrap();
    let mut ids = vec![];
    for (i, source) in sources.iter().enumerate() {
        let name = if sources.len() == 1 {
            "Input".into()
        } else {
            format!("Input {i}")
        };
        ids.push(
            doc.add_geometry_with_attributes(
                Geometry::SubD((*source).clone()),
                ObjectAttributes::on_layer(layer)
                    .with_name(name)
                    .with_object_color(ColorRgb::new(17 + i as u8, 43 + i as u8, 91 + i as u8)),
            )
            .unwrap(),
        );
    }
    doc.add_group(Some("SubD offset group".into()), ids.iter().copied())
        .unwrap();
    doc.clear_history().unwrap();
    (doc, ids)
}
pub fn expected(value: &Value) -> Value {
    viboceros_oracle::subd_join::expected(value)
}
pub fn options(op: &Value) -> viboceros_command::offset_subd::Options {
    viboceros_command::offset_subd::Options {
        distance: op["distance"].as_f64().unwrap(),
        solid: op["solid"].as_bool().unwrap(),
        both_sides: op["both_sides"].as_bool().unwrap(),
        delete_input: op["delete_input"].as_bool().unwrap(),
        flip_all: op["flip"].as_bool().unwrap(),
    }
}
