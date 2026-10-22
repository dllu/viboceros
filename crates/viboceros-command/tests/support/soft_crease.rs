//! Immutable native input files and the original live sharpness seed recipe.
use serde_json::Value;
use std::path::Path;
use viboceros_document::{Document, Geometry, ObjectId};
pub type Sources = Vec<(ObjectId, String, Geometry)>;
pub type Targets = Vec<(ObjectId, usize)>;
pub fn setup(root: &Path, family: &str, id: &str, value: &Value) -> (Document, Sources, Targets) {
    let path = root.join(format!(
        "tools/rhino_oracle/fixtures/{family}/source/{id}.3dm"
    ));
    let (mut document, _, _, _) =
        viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let objects = document
        .objects()
        .filter(|o| matches!(o.geometry(), Geometry::SubD(_)))
        .map(|o| {
            (
                o.id(),
                o.attributes().name().unwrap().to_owned(),
                o.geometry().clone(),
            )
        })
        .collect::<Vec<_>>();
    let mut targets = Vec::new();
    for (oid, name, geometry) in &objects {
        let before = value["before"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["name"] == *name)
            .unwrap();
        let source = before["source"].as_u64().unwrap() as usize;
        let input = &value["input"]["sources"][source];
        let Geometry::SubD(subd) = geometry else {
            unreachable!()
        };
        if let Some(weight) = input["soft"].as_f64() {
            let pairs = input
                .get("seed_edges")
                .unwrap_or(&input["edges"])
                .as_array()
                .unwrap();
            let edits = subd
                .edges()
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    pairs.iter().any(|p| {
                        let (a, b) = (p[0].as_u64().unwrap() as u32, p[1].as_u64().unwrap() as u32);
                        **e == [a, b] || **e == [b, a]
                    })
                })
                .map(|(e, _)| (e, [weight; 2]))
                .collect::<Vec<_>>();
            let restored = subd.try_with_soft_crease_weights(&edits).unwrap().0;
            document
                .replace_object_geometries_preserving_control_selection([(
                    *oid,
                    Geometry::SubD(restored),
                )])
                .unwrap();
        }
        targets.extend(
            before["selected_edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| (*oid, e.as_u64().unwrap() as usize)),
        );
    }
    (document, objects, targets)
}
