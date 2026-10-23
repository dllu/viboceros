//! Shared immutable crease inputs and independent native geometry checks.
#[path = "mesh_command.rs"]
pub mod mesh_command;
use super::{mesh_native, subd_native};
use serde_json::{Value, json};
use std::path::Path;
use viboceros_command::crease::{CreaseMode, CreaseTarget};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::Point3;

pub fn rows() -> Vec<Value> {
    let mut result = Vec::new();
    for (source, count) in [
        (
            include_str!("../../../../tools/rhino_oracle/observations/crease_main_v3.json"),
            62,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/crease_extra.json"),
            30,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/crease_policy.json"),
            12,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/crease_mixed.json"),
            10,
        ),
        (
            include_str!("../../../../tools/rhino_oracle/observations/crease_overlap.json"),
            4,
        ),
    ] {
        let capture: Value = serde_json::from_str(source).unwrap();
        assert_eq!(capture["results"].as_array().unwrap().len(), count);
        result.extend(
            capture["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["value"]["success"] == true)
                .cloned(),
        );
    }
    result
}

pub fn point(v: &Value) -> Point3 {
    Point3::try_new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
    .unwrap()
}
pub fn setup(row: &Value) -> (Document, Vec<ObjectId>) {
    let family = if row["id"].as_str().unwrap().starts_with("mixed_") {
        "crease_mixed"
    } else if row["id"].as_str().unwrap().starts_with("overlap_") {
        "crease_overlap"
    } else if row["id"].as_str().unwrap().starts_with("policy_") {
        "crease_policy"
    } else if row["id"].as_str().unwrap().starts_with("extra_") {
        "crease_extra"
    } else {
        "crease_main_v3"
    };
    setup_from_family(row, family)
}
pub fn setup_from_family(row: &Value, family: &str) -> (Document, Vec<ObjectId>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .expect("workspace fixtures");
    let path = root
        .join("tools/rhino_oracle/fixtures")
        .join(family)
        .join(format!("{}.3dm", row["id"].as_str().unwrap()));
    let (mut doc, _, _, _) =
        viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let native = &row["value"];
    let layer = doc
        .add_layer(
            native["before"][0]["layer"].as_str().unwrap(),
            ColorRgb::new(0, 0, 255),
        )
        .unwrap();
    let mut source_ids = std::collections::BTreeMap::new();
    for source in native["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["source"].is_number())
    {
        let name = source["name"].as_str().unwrap();
        let id = doc
            .objects()
            .find(|o| o.attributes().name() == Some(name))
            .unwrap()
            .id();
        doc.set_objects_layer([id], layer).unwrap();
        doc.set_objects_color([id], Some(ColorRgb::new(17, 43, 91)))
            .unwrap();
        source_ids.insert(source["source"].as_u64().unwrap() as usize, id);
    }
    let ids = source_ids.into_values().collect::<Vec<_>>();
    doc.add_group(
        Some(native["before"][0]["groups"][0].as_str().unwrap().into()),
        ids.iter().copied(),
    )
    .unwrap();
    let current = doc
        .add_layer(
            native["before"].as_array().unwrap().last().unwrap()["layer"]
                .as_str()
                .unwrap(),
            ColorRgb::new(0, 128, 0),
        )
        .unwrap();
    doc.set_current_layer(current).unwrap();
    let witness = doc
        .objects()
        .find(|o| o.attributes().name() == Some("Undo witness"))
        .unwrap()
        .id();
    doc.delete_object(witness).unwrap();
    doc.clear_selection();
    doc.clear_history().unwrap();
    doc.begin_transaction("Undo witness").unwrap();
    let witness = doc
        .add_geometry(Geometry::Point(point(
            &native["before"].as_array().unwrap().last().unwrap()["point"],
        )))
        .unwrap();
    doc.set_object_names([(witness, Some("Undo witness".into()))])
        .unwrap();
    doc.commit_transaction().unwrap();
    for source in native["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["grips_on"] == true)
    {
        let id = ids[source["source"].as_u64().unwrap() as usize];
        if native["input"]["edit_points"] == true {
            doc.enable_edit_points([id]).unwrap();
        } else {
            doc.enable_control_points([id]).unwrap();
        }
        doc.select_control_points(
            source["selected_grips"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| viboceros_document::ControlPointId {
                    object: id,
                    index: i.as_u64().unwrap() as usize,
                }),
            SelectionMode::Add,
        )
        .unwrap();
    }
    doc.select_objects_direct(
        native["before"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["selected"] == true)
            .map(|s| ids[s["source"].as_u64().unwrap() as usize]),
        SelectionMode::Add,
    )
    .unwrap();
    (doc, ids)
}
pub fn targets(doc: &Document, ids: &[ObjectId], v: &Value) -> Vec<CreaseTarget> {
    let mut targets = Vec::new();
    for row in v["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["source"].is_number())
    {
        let source = row["source"].as_u64().unwrap() as usize;
        let id = ids[source];
        match doc.object(id).unwrap().geometry() {
            Geometry::SubD(_) => {
                targets.extend(
                    row["selected_vertices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .chain(row["selected_grips"].as_array().into_iter().flatten())
                        .map(|i| CreaseTarget::Vertex(id, i.as_u64().unwrap() as usize)),
                );
                targets.extend(
                    row["selected_edges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| CreaseTarget::Edge(id, i.as_u64().unwrap() as usize)),
                );
            }
            Geometry::Mesh(mesh) => {
                let edges = mesh.topology_edge_points();
                let recipe = &v["input"]["sources"][source];
                for edge in recipe["edges"].as_array().unwrap() {
                    let a = point(&recipe["vertices"][edge[0].as_u64().unwrap() as usize]);
                    let b = point(&recipe["vertices"][edge[1].as_u64().unwrap() as usize]);
                    let i = edges
                        .iter()
                        .position(|&[x, y]| (x == a && y == b) || (x == b && y == a))
                        .unwrap();
                    targets.push(CreaseTarget::Edge(id, i));
                }
            }
            _ => panic!("crease source type"),
        }
    }
    targets
}
pub fn mode(v: &Value) -> CreaseMode {
    if v["input"]["command"] == "Crease" {
        CreaseMode::Add
    } else {
        CreaseMode::Remove
    }
}
pub fn check(doc: &Document, rows: &Value, selection: bool) -> f64 {
    check_impl(doc, rows, selection, true)
}
pub fn check_metadata(doc: &Document, rows: &Value, selection: bool) {
    check_impl(doc, rows, selection, false);
}
fn check_impl(doc: &Document, rows: &Value, selection: bool, geometry: bool) -> f64 {
    let rows = rows.as_array().unwrap();
    assert_eq!(doc.objects().len(), rows.len());
    let mut maximum = 0f64;
    for (object, row) in doc.objects().zip(rows) {
        assert_eq!(
            object.attributes().name(),
            row["name"].as_str(),
            "object order"
        );
        assert_eq!(
            doc.layer(object.attributes().layer_id()).unwrap().name(),
            row["layer"].as_str().unwrap()
        );
        if selection {
            assert_eq!(
                doc.is_selected(object.id()),
                row["selected"].as_bool().unwrap()
            );
        }
        // The main producer predates grip capture. Later immutable families
        // record both fields, including all six dedicated grip workflows.
        if let Some(grips_on) = row.get("grips_on") {
            assert_eq!(
                doc.grip_mode(object.id()).is_some(),
                grips_on.as_bool().unwrap()
            );
            let grips = doc
                .selected_logical_control_points()
                .filter(|(g, _)| g.object == object.id())
                .map(|(g, _)| g.index)
                .collect::<Vec<_>>();
            assert_eq!(
                grips,
                row["selected_grips"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .collect::<Vec<_>>()
            );
        }
        let groups = object
            .group_ids()
            .iter()
            .map(|&id| doc.group(id).unwrap().name().unwrap())
            .collect::<Vec<_>>();
        let expected = row["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(groups, expected);
        match object.geometry() {
            Geometry::SubD(subd) if geometry => {
                let report = subd_native::compare(subd, &row["subd"]);
                maximum = maximum
                    .max(
                        report["maximum_control_or_limit_point_error"]
                            .as_f64()
                            .unwrap(),
                    )
                    .max(report["maximum_sampled_surface_error"].as_f64().unwrap());
            }
            Geometry::Mesh(mesh) if geometry => {
                let mut expected = row["mesh"].clone();
                // RhinoCommon Color.A is opacity; the model stores OpenNURBS
                // transparency bytes. Retain the unconverted capture unchanged.
                if let Some(colors) = expected["colors"].as_array_mut() {
                    for color in colors {
                        color[3] = json!(255 - color[3].as_u64().unwrap());
                    }
                }
                // An empty native n-gon slot is a removed region, not a polygon.
                let ngons = expected["ngons"].as_array_mut().unwrap();
                ngons.retain(|n| {
                    !(n["vertices"].as_array().unwrap().is_empty()
                        && n["faces"].as_array().unwrap().is_empty())
                });
                let report = mesh_native::compare_mesh(mesh, &expected, 0);
                maximum = maximum.max(report["maximum_vertex_error"].as_f64().unwrap());
            }
            Geometry::Point(p) => {
                maximum = maximum.max(p.distance_to(point(&row["point"])).unwrap())
            }
            Geometry::Mesh(_) | Geometry::SubD(_) => {}
            _ => panic!("unexpected crease geometry"),
        }
    }
    assert!(maximum <= 1e-6, "numeric error {maximum}");
    maximum
}
