use super::native_curve_comparison as curves;
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_document::{Document, Object, ObjectId};

pub fn check(
    doc: &Document,
    reference: &Value,
    source: &Object,
    name: &str,
    mapping: &mut BTreeMap<String, ObjectId>,
) -> f64 {
    let entries = reference.as_array().unwrap();
    assert_eq!(doc.objects().len(), entries.len(), "{name}: object count");
    let mut max = 0_f64;
    for entry in entries {
        let key = entry["id"].as_str().unwrap();
        let id = if let Some(id) = mapping.get(key) {
            *id
        } else {
            let point = viboceros_geometry::Point3::try_from(
                serde_json::from_value::<[f64; 3]>(entry["geometry"]["samples"][0].clone())
                    .unwrap(),
            )
            .unwrap();
            let target = doc
                .objects()
                .filter(|o| !mapping.values().any(|id| *id == o.id()))
                .min_by(|a, b| {
                    let gap = |o: &Object| {
                        o.geometry()
                            .curve_ref()
                            .unwrap()
                            .start_point()
                            .unwrap()
                            .distance_to(point)
                            .unwrap()
                    };
                    gap(a).total_cmp(&gap(b))
                })
                .unwrap_or_else(|| panic!("{name}: missing output"));
            mapping.insert(key.to_owned(), target.id());
            target.id()
        };
        let object = doc.object(id).unwrap();
        assert_eq!(
            object.attributes(),
            source.attributes(),
            "{name}: attributes"
        );
        assert_eq!(object.group_ids(), source.group_ids(), "{name}: groups");
        assert_eq!(
            doc.is_selected(id),
            entry["selected"].as_bool().unwrap(),
            "{name}: selection"
        );
        assert_eq!(
            serde_json::to_value(object.geometry_user_text()).unwrap(),
            entry["geometry_text"],
            "{name}: geometry text"
        );
        max = max.max(curves::compare(
            object.geometry().curve_ref().unwrap(),
            &entry["geometry"],
            name,
        ));
    }
    max
}
