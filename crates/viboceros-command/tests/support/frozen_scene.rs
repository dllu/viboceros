//! Shared saved-document setup for native command and getter replays.
use serde_json::Value;
use viboceros_document::{ColorRgb, Document, Geometry, GeometrySnapshot, ObjectId, SelectionMode};
use viboceros_geometry::Point3;

pub struct Scene {
    pub doc: Document,
    pub ids: Vec<ObjectId>,
    pub witness: ObjectId,
    pub sources: Vec<GeometrySnapshot>,
}

pub fn setup(path: &std::path::Path, value: &Value, output_layer: &str) -> Scene {
    let (mut doc, _, _, _) =
        viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let mut ids = Vec::new();
    for source in value["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["role"] == "source")
    {
        let name = source["name"].as_str().unwrap();
        let id = doc
            .objects()
            .find(|o| o.attributes().name() == Some(name))
            .unwrap()
            .id();
        let layer = doc
            .add_layer(
                source["layer"].as_str().unwrap(),
                ColorRgb::new(91, 109, 121),
            )
            .unwrap();
        doc.set_objects_layer([id], layer).unwrap();
        let color = &source["color"];
        doc.set_objects_color(
            [id],
            Some(ColorRgb::new(
                color[0].as_u64().unwrap() as u8,
                color[1].as_u64().unwrap() as u8,
                color[2].as_u64().unwrap() as u8,
            )),
        )
        .unwrap();
        ids.push(id);
    }
    let mut groups = std::collections::BTreeSet::new();
    for source in value["before"].as_array().unwrap() {
        for group in source["groups"].as_array().unwrap() {
            groups.insert(group.as_str().unwrap().to_owned());
        }
    }
    for name in groups {
        let members = value["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                r["role"] == "source" && r["groups"].as_array().unwrap().iter().any(|g| g == &name)
            })
            .map(|r| ids[r["source"].as_u64().unwrap() as usize])
            .collect::<Vec<_>>();
        doc.add_group(Some(name), members).unwrap();
    }
    let current = doc
        .add_layer(output_layer, ColorRgb::new(91, 109, 121))
        .unwrap();
    doc.set_current_layer(current).unwrap();
    doc.clear_selection();
    doc.clear_history().unwrap();
    doc.begin_transaction("Undo witness").unwrap();
    let witness = doc
        .add_geometry(Geometry::Point(
            Point3::try_from(std::array::from_fn(|i| {
                value["before"].as_array().unwrap().last().unwrap()["point"][i]
                    .as_f64()
                    .unwrap()
            }))
            .unwrap(),
        ))
        .unwrap();
    doc.set_object_names([(witness, Some("Undo witness".into()))])
        .unwrap();
    doc.commit_transaction().unwrap();
    let selected = value["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["selected"] == true)
        .map(|r| ids[r["source"].as_u64().unwrap() as usize])
        .collect::<Vec<_>>();
    doc.select_objects_direct(selected, SelectionMode::Replace)
        .unwrap();
    let sources = ids
        .iter()
        .map(|id| doc.object(*id).unwrap().geometry_snapshot().clone())
        .collect();
    Scene {
        doc,
        ids,
        witness,
        sources,
    }
}
