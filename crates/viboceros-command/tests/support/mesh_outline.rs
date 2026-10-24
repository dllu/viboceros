//! Saved sources, camera context, attributes and document lifecycle for outlines.
#[allow(dead_code)]
#[path = "../../../viboceros-geometry/tests/support/mesh_outline.rs"]
pub mod reference;
use reference::{frame, point, vector};
use serde_json::{Value, json};
use viboceros_command::{CommandContext, picking::Projection};
use viboceros_document::{ColorRgb, Document, Geometry, GeometrySnapshot, ObjectId, SelectionMode};
use viboceros_geometry::{Polyline3, Tolerance, TriangleMesh};
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

pub fn rows() -> Vec<Value> {
    let mut rows = Vec::new();
    for text in [
        include_str!("../../../../tools/rhino_oracle/observations/mesh_outline_lifecycle.json"),
        include_str!("../../../../tools/rhino_oracle/observations/mesh_outline_quads.json"),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        rows.extend(data["results"].as_array().unwrap().iter().cloned());
    }
    assert_eq!(rows.len(), 39);
    rows
}

pub fn view(value: &Value) -> ThreeDmNamedView {
    view_at(value, "after")
}

pub fn view_at(value: &Value, stage: &str) -> ThreeDmNamedView {
    let v = value
        .get("views")
        .map_or(&value["view"], |views| &views[stage]);
    ThreeDmNamedView {
        name: "Native outline input".into(),
        projection: if v["perspective"] == true {
            ThreeDmProjection::Perspective
        } else {
            ThreeDmProjection::Parallel
        },
        camera_location: point(&v["location"]),
        camera_direction: vector(&v["direction"]),
        camera_up: vector(&v["up"]),
        target: Some(point(&v["target"])),
        construction_plane: frame(&value["plane"]),
        frustum: std::array::from_fn(|i| v["frustum"][i].as_f64().unwrap()),
        screen_port: [0, 800, 0, 600],
    }
}

pub fn context(value: &Value) -> (CommandContext, Projection) {
    let view = view(value);
    (
        CommandContext {
            construction_plane: view.construction_plane,
        },
        Projection::from_view(&view).unwrap(),
    )
}

pub struct Scene {
    pub doc: Document,
    pub ids: Vec<ObjectId>,
    pub witness: ObjectId,
    pub sources: Vec<GeometrySnapshot>,
}

pub fn setup(row: &Value) -> Scene {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let path = root
        .join("tools/rhino_oracle/fixtures")
        .join(
            if row["id"].as_str().unwrap().starts_with("outline_quads_") {
                "mesh_outline_quads"
            } else {
                "mesh_outline_lifecycle"
            },
        )
        .join(format!("{}.3dm", row["id"].as_str().unwrap()));
    let (mut doc, _, _, _) =
        viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let value = &row["value"];
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
        .add_layer("Outline output", ColorRgb::new(91, 109, 121))
        .unwrap();
    doc.set_current_layer(current).unwrap();
    doc.clear_selection();
    doc.clear_history().unwrap();
    doc.begin_transaction("Undo witness").unwrap();
    let witness = doc
        .add_geometry(Geometry::Point(point(
            &value["before"].as_array().unwrap().last().unwrap()["point"],
        )))
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

pub fn snapshot(scene: &Scene) -> Value {
    json!(scene.doc.objects().map(|obj|{
        let a=obj.attributes();let color=a.object_color();
        let source=scene.ids.iter().position(|id|*id==obj.id());
        let mut groups=obj.group_ids().iter().map(|id|scene.doc.group(*id).unwrap().name().unwrap().to_owned()).collect::<Vec<_>>();groups.sort();
        let mut row=json!({"source":source,"role":if source.is_some(){"source"}else if obj.id()==scene.witness{"witness"}else{"output"},
            "selected":scene.doc.is_selected(obj.id()),"name":a.name(),"layer":scene.doc.layer(a.layer_id()).unwrap().name(),
            "color":[color.red,color.green,color.blue],"color_source":a.color_source()as u8,"groups":groups});
        if let Geometry::Polyline(p)=obj.geometry(){row["polyline"]=json!(p.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>());}
        row
    }).collect::<Vec<_>>())
}

pub fn check(scene: &Scene, value: &Value, stage: &str) -> (Value, bool, Option<f64>) {
    let actual = snapshot(scene);
    let expected = value[stage].as_array().unwrap();
    let mut polylines = Vec::new();
    let mut native = Vec::new();
    assert_eq!(
        actual.as_array().unwrap().len(),
        expected.len(),
        "object count at {stage}"
    );
    for (a, n) in actual.as_array().unwrap().iter().zip(expected) {
        for key in [
            "source",
            "role",
            "selected",
            "name",
            "layer",
            "color",
            "color_source",
            "groups",
        ] {
            assert_eq!(a[key], n[key], "{stage} {key}");
        }
        if a["role"] == "output" {
            polylines.push(
                Polyline3::try_new(
                    a["polyline"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(point)
                        .collect(),
                    Tolerance::MESH_VALIDATION,
                )
                .unwrap(),
            );
            native.push(n["polyline"].clone());
        }
    }
    for (id, original) in scene.ids.iter().zip(&scene.sources) {
        assert!(
            scene
                .doc
                .object(*id)
                .unwrap()
                .geometry_snapshot()
                .shares_storage_with(original),
            "outline query mutated a source"
        );
    }
    let error = reference::boundary_error(&polylines, &json!(native), 1e-7);
    (actual, error.is_some_and(|e| e <= 1e-7), error)
}

pub fn native_render_control(value: &Value) -> (Value, Option<f64>) {
    let meshes = value["render_meshes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .map(reference::mesh)
        .collect::<Vec<_>>();
    let aggregate = TriangleMesh::try_append(&meshes.iter().collect::<Vec<_>>()).unwrap();
    let (context, projection) = context(value);
    let actual = aggregate
        .outlines(
            projection
                .mesh_outline_projection(context.construction_plane)
                .unwrap(),
            Tolerance::MESH_VALIDATION,
        )
        .unwrap();
    let expected = json!(
        value["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["role"] == "output")
            .map(|o| o["polyline"].clone())
            .collect::<Vec<_>>()
    );
    let error = reference::boundary_error(&actual, &expected, 1e-7);
    (
        json!(
            actual
                .iter()
                .map(|p| p
                    .vertices()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>()
        ),
        error,
    )
}
