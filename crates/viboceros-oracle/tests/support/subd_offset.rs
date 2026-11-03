pub use super::common::{cases_for, compare};
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::SubD;
pub fn sources() -> BTreeMap<String, SubD> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source.3dm");
    let mut sources = BTreeMap::new();
    for (op, native) in cases_for("subd_offset_sdk") {
        let key = op["scene"].as_str().unwrap();
        if sources.contains_key(key) {
            continue;
        }
        let bytes = native["archive"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| u8::try_from(b.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>();
        std::fs::write(&path, bytes).unwrap();
        let (doc, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        let Geometry::SubD(s) = doc.objects().next().unwrap().geometry() else {
            panic!()
        };
        sources.insert(key.to_string(), s.clone());
    }
    sources
}
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
