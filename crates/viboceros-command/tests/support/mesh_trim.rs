//! Saved physical MeshTrim picks; macro-only preselection captures are excluded.
use super::frozen_scene;
use serde_json::Value;
use viboceros_document::{Geometry, ObjectId};
use viboceros_geometry::Point3;

fn root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap()
}
pub fn rows() -> Vec<Value> {
    let mut rows = vec![];
    for family in ["mouse", "controls"] {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(root().join(format!(
                "tools/rhino_oracle/observations/mesh_trim_{family}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        for mut row in data["results"].as_array().unwrap().iter().cloned() {
            if row["value"]["input"]["preselect"] == true {
                continue;
            }
            row["family"] = family.into();
            rows.push(row);
        }
    }
    rows
}
pub fn setup(row: &Value) -> frozen_scene::Scene {
    frozen_scene::setup(
        &root().join(format!(
            "tools/rhino_oracle/fixtures/mesh_trim_{}/{}.3dm",
            row["family"].as_str().unwrap(),
            row["id"].as_str().unwrap()
        )),
        &row["value"],
        "Boolean output",
    )
}
pub fn cutters(row: &Value, ids: &[ObjectId]) -> Vec<ObjectId> {
    row["value"]["input"]["first"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| ids[i.as_u64().unwrap() as usize])
        .collect()
}
pub fn picks(row: &Value) -> Vec<Point3> {
    row["value"]["input"]["trim_picks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Point3::try_from(std::array::from_fn(|i| p[i].as_f64().unwrap())).unwrap())
        .collect()
}
pub fn target(doc: &viboceros_document::Document, point: Point3) -> ObjectId {
    doc.objects()
        .filter(|o| o.attributes().name() == Some("Source 0"))
        .filter_map(|o| {
            let Geometry::Mesh(mesh) = o.geometry() else {
                return None;
            };
            (0..mesh.faces().len())
                .map(|f| mesh.closest_point_on_face(f, point).unwrap())
                .min_by(|&a, &b| point.compare_distances(a, b))
                .map(|p| (o.id(), p))
        })
        .min_by(|&(_, a), &(_, b)| point.compare_distances(a, b))
        .unwrap()
        .0
}
pub fn command(row: &Value, cutters: &[ObjectId], target: ObjectId, point: Point3) -> String {
    let [x, y, z] = point.to_array();
    let mut result = format!("MeshTrim Target={target} Point={x},{y},{z}");
    if !cutters.is_empty() {
        result.push_str(&format!(
            " Cutters={}",
            cutters
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if let Some(plane) = row["value"]["input"]["infinite_plane"].as_str() {
        result.push_str(&format!(" InfinitePlane={plane}"));
    }
    result
}
pub const GAPS: &[&str] = &["mesh_trim_controls_plane"];
