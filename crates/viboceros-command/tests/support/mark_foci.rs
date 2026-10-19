use super::native_curve_comparison as curves;
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_document::{Document, Geometry, Object, ObjectAttributes, ObjectId};
use viboceros_geometry::Point3;
fn point(value: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
fn gap(object: &Object, entry: &Value) -> f64 {
    let g = &entry["geometry"];
    match (object.geometry(), g["type"].as_str().unwrap()) {
        (Geometry::Point(p), "point") => p.distance_to(point(&g["point"])).unwrap(),
        (Geometry::Line(line), "line") => {
            let samples = g["samples"].as_array().unwrap();
            let a = point(&samples[0]);
            let b = point(samples.last().unwrap());
            let forward = line
                .start()
                .distance_to(a)
                .unwrap()
                .max(line.end().distance_to(b).unwrap());
            let reverse = line
                .start()
                .distance_to(b)
                .unwrap()
                .max(line.end().distance_to(a).unwrap());
            forward
                .min(reverse)
                .max((*line.domain().end() - g["domain"][1].as_f64().unwrap()).abs())
        }
        _ => f64::INFINITY,
    }
}
pub fn check(
    doc: &Document,
    reference: &Value,
    sources: &[Object],
    name: &str,
    mapping: &mut BTreeMap<String, ObjectId>,
) -> f64 {
    let entries = reference.as_array().unwrap();
    assert_eq!(doc.objects().len(), entries.len(), "{name}: object count");
    let mut maximum = 0_f64;
    for entry in entries {
        let native = entry["id"].as_str().unwrap();
        let id = if let Some(id) = mapping.get(native) {
            *id
        } else {
            let target = doc
                .objects()
                .filter(|o| !mapping.values().any(|id| *id == o.id()))
                .min_by(|a, b| gap(a, entry).total_cmp(&gap(b, entry)))
                .unwrap();
            mapping.insert(native.into(), target.id());
            target.id()
        };
        let object = doc.object(id).unwrap();
        if let Some(source) = sources.iter().find(|s| s.id() == id) {
            assert_eq!(object, source, "{name}: immutable source");
            maximum = maximum.max(curves::compare(
                object.geometry().curve_ref().unwrap(),
                &entry["geometry"],
                name,
            ));
        } else {
            assert_eq!(
                object.attributes(),
                &ObjectAttributes::on_layer(doc.current_layer_id()),
                "{name}: annotation attributes"
            );
            assert!(object.group_ids().is_empty());
            assert!(object.geometry_user_text().is_empty());
            maximum = maximum.max(gap(object, entry));
        }
        assert_eq!(
            doc.is_selected(id),
            entry["selected"].as_bool().unwrap(),
            "{name}: selection"
        );
    }
    maximum
}
pub fn initial_mapping(sources: &[Object]) -> BTreeMap<String, ObjectId> {
    sources
        .iter()
        .map(|s| (s.id().to_string(), s.id()))
        .collect()
}
