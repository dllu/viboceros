use super::native_curve_comparison as curves;
use serde_json::Value;
use std::collections::BTreeMap;
use viboceros_document::{Document, Object, ObjectId};

pub fn check(
    doc: &Document,
    reference: &Value,
    sources: &[Object],
    name: &str,
    mapping: &mut BTreeMap<String, ObjectId>,
) -> f64 {
    let entries = reference.as_array().unwrap();
    assert_eq!(doc.objects().len(), entries.len(), "{name}: object count");
    let mut max = 0_f64;
    for entry in entries {
        let native_id = entry["id"].as_str().unwrap();
        let id = if let Some(id) = mapping.get(native_id) {
            *id
        } else {
            let sample = &entry["geometry"]["samples"][0];
            let point = viboceros_geometry::Point3::try_from(
                serde_json::from_value::<[f64; 3]>(sample.clone()).unwrap(),
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
                .unwrap_or_else(|| panic!("{name}: missing copy"));
            mapping.insert(native_id.to_string(), target.id());
            target.id()
        };
        let object = doc.object(id).unwrap();
        if let Some(original) = sources.iter().find(|o| o.id() == id) {
            assert_eq!(
                object.attributes(),
                original.attributes(),
                "{name}: source attributes"
            );
            assert_eq!(
                object.group_ids(),
                original.group_ids(),
                "{name}: source groups"
            );
        } else {
            assert_eq!(
                object.attributes(),
                &viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id()),
                "{name}: fresh copy attributes"
            );
            assert!(object.group_ids().is_empty(), "{name}: fresh copy groups");
        }
        assert_eq!(
            object.attributes().name(),
            entry["name"].as_str(),
            "{name}: name"
        );
        assert_eq!(
            serde_json::to_value(object.attributes().user_text()).unwrap(),
            entry["user_text"],
            "{name}: attribute text"
        );
        assert_eq!(
            serde_json::to_value(object.geometry_user_text()).unwrap(),
            entry["geometry_text"],
            "{name}: geometry text"
        );
        assert_eq!(
            doc.is_selected(id),
            entry["selected"].as_bool().unwrap(),
            "{name}: selection for {}",
            entry["name"]
        );
        max = max.max(curves::compare(
            object.geometry().curve_ref().unwrap(),
            &entry["geometry"],
            name,
        ));
    }
    max
}

pub fn initial_mapping(sources: &[Object]) -> BTreeMap<String, ObjectId> {
    sources
        .iter()
        .map(|o| (o.id().to_string(), o.id()))
        .collect()
}
