#[allow(dead_code)]
#[path = "support/subd.rs"]
mod subd_native;

#[allow(dead_code)]
#[path = "../../viboceros-geometry/tests/support/subdivision_native.rs"]
mod mesh_native;
use serde_json::{Value, json};
use std::path::Path;
use viboceros_command::{
    CommandRegistry,
    crease::{CreaseMode, CreaseSelection, CreaseTarget},
};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectId, SelectionMode};
use viboceros_geometry::Point3;

fn rows() -> Vec<Value> {
    let mut result = Vec::new();
    for (source, count) in [
        (
            include_str!("../../../tools/rhino_oracle/observations/crease_main_v3.json"),
            62,
        ),
        (
            include_str!("../../../tools/rhino_oracle/observations/crease_extra.json"),
            30,
        ),
        (
            include_str!("../../../tools/rhino_oracle/observations/crease_policy.json"),
            12,
        ),
        (
            include_str!("../../../tools/rhino_oracle/observations/crease_mixed.json"),
            10,
        ),
        (
            include_str!("../../../tools/rhino_oracle/observations/crease_overlap.json"),
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

fn point(v: &Value) -> Point3 {
    Point3::try_new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
    .unwrap()
}
fn setup(row: &Value) -> (Document, Vec<ObjectId>) {
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
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/rhino_oracle/fixtures")
        .join(family)
        .join(format!("{}.3dm", row["id"].as_str().unwrap()));
    let (mut doc, _, _, _) =
        viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let native = &row["value"];
    let layer = doc
        .add_layer("Crease input", ColorRgb::new(0, 0, 255))
        .unwrap();
    let mut ids = Vec::new();
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
        ids.push(id);
    }
    doc.add_group(Some("Crease assembly".into()), ids.iter().copied())
        .unwrap();
    let current = doc
        .add_layer("Crease current", ColorRgb::new(0, 128, 0))
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
        doc.enable_control_points([id]).unwrap();
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
    (doc, ids)
}
fn targets(doc: &Document, ids: &[ObjectId], v: &Value) -> Vec<CreaseTarget> {
    let mut targets = Vec::new();
    for (source, row) in v["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["source"].is_number())
        .enumerate()
    {
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
fn mode(v: &Value) -> CreaseMode {
    if v["input"]["command"] == "Crease" {
        CreaseMode::Add
    } else {
        CreaseMode::Remove
    }
}
fn check(doc: &Document, rows: &Value, selection: bool) -> f64 {
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
            Geometry::SubD(subd) => {
                let report = subd_native::compare(subd, &row["subd"]);
                maximum = maximum
                    .max(
                        report["maximum_control_or_limit_point_error"]
                            .as_f64()
                            .unwrap(),
                    )
                    .max(report["maximum_sampled_surface_error"].as_f64().unwrap());
            }
            Geometry::Mesh(mesh) => {
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
            _ => panic!("unexpected crease geometry"),
        }
    }
    assert!(maximum <= 1e-6, "numeric error {maximum}");
    maximum
}

#[test]
fn staged_crease_edits_match_native_controls_facets_attributes_order_and_undo() {
    let rows = rows();
    assert_eq!(rows.len(), 110);
    let mut failures = Vec::new();
    for row in rows {
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let v = &row["value"];
            let (mut doc, ids) = setup(&row);
            let mut maximum = check(&doc, &v["before"], true);
            let original = ids
                .iter()
                .map(|&id| doc.object(id).unwrap().attributes().clone())
                .collect::<Vec<_>>();
            let prepared = CreaseSelection::prepare(&doc, mode(v), targets(&doc, &ids, v)).unwrap();
            let out = prepared.commit(&mut doc).unwrap();
            for (&id, attributes) in ids.iter().zip(&original) {
                assert_eq!(doc.object(id).unwrap().attributes(), attributes);
            }
            maximum = maximum.max(check(&doc, &v["after"], true));
            let native_changed = v["undo"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["name"] == "Undo witness");
            assert_eq!(!out.processed_objects.is_empty(), native_changed);
            assert_eq!(
                doc.undo_label(),
                Some(if native_changed {
                    mode(v).command()
                } else {
                    "Undo witness"
                })
            );
            // The producer appends Cancel after observing command completion.
            // Rhino turns grips off there; Undo restores the command's input
            // grips, while Redo restores this post-cleanup display state.
            if v["input"]["grips"] == true {
                doc.disable_control_points();
            }
            maximum = maximum.max(check(&doc, &v["after_script"], true));
            doc.undo().unwrap();
            maximum = maximum.max(check(&doc, &v["undo"], true));
            doc.redo().unwrap();
            maximum = maximum.max(check(&doc, &v["redo"], true));
            println!(
                "CREASE_COMMAND {}",
                json!({"id":row["id"],"maximum_error":maximum,"identity":true,"attributes":true,"history":true,"processed_objects":out.processed_objects.len(),"native_empty_ngon_slots":v["after"].as_array().unwrap().iter().flat_map(|r|r["mesh"]["ngons"].as_array().into_iter().flatten()).filter(|n|n["vertices"].as_array().unwrap().is_empty()&&n["faces"].as_array().unwrap().is_empty()).count()})
            );
        }));
        if run.is_err() {
            failures.push(row["id"].as_str().unwrap().to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "native crease differences: {failures:?}"
    );
}

#[test]
fn typed_crease_components_use_the_registered_commands_and_atomic_history() {
    for name in [
        "main_crease_subd_edge",
        "main_removecrease_subd_creased_edge",
        "main_crease_subd_vertex",
        "main_removecrease_subd_corner",
        "main_crease_mesh_edge",
        "main_removecrease_mesh_seam",
    ] {
        let row = rows().into_iter().find(|r| r["id"] == name).unwrap();
        let v = &row["value"];
        let (mut doc, ids) = setup(&row);
        let selected = targets(&doc, &ids, v);
        let edges = selected
            .iter()
            .filter_map(|t| match t {
                CreaseTarget::Edge(_, i) => Some(i.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let vertices = selected
            .iter()
            .filter_map(|t| match t {
                CreaseTarget::Vertex(_, i) => Some(i.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        let mut command = mode(v).command().to_string();
        if !edges.is_empty() {
            command.push_str(&format!(" Edges={}", edges.join(",")));
        }
        if !vertices.is_empty() {
            command.push_str(&format!(" Vertices={}", vertices.join(",")));
        }
        CommandRegistry::with_builtins()
            .execute(&mut doc, &command)
            .unwrap();
        check(&doc, &v["after"], true);
        doc.undo().unwrap();
        check(&doc, &v["undo"], false);
        doc.redo().unwrap();
        check(&doc, &v["redo"], true);
    }
}
