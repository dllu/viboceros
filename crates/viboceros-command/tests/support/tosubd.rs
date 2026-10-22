//! Fixed native inputs; multi-face sources retain recorded underlying surface tables.
use super::geometry_native as coordinates;
use super::subd_native;
use serde_json::{Value, json};
use viboceros_command::to_subd::{ToSubDOptions, ToSubDTarget};
use viboceros_document::{ColorRgb, Document, Geometry, GeometrySnapshot, ObjectId, SelectionMode};
use viboceros_geometry::{Brep, NurbsSurface, SubDFromSurfaceMethod, Tolerance, WeightedPoint3};
pub struct Scene {
    pub doc: Document,
    pub ids: Vec<ObjectId>,
    pub originals: Vec<GeometrySnapshot>,
}
fn root() -> std::path::PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if manifest.join("tools/rhino_oracle").is_dir() {
        manifest.into()
    } else {
        manifest.join("../..")
    }
}
pub fn rows() -> Vec<Value> {
    let families = [
        "tosubd",
        "tosubd_surface_details",
        "tosubd_surface_edges",
        "tosubd_features",
        "tosubd_poles",
        "tosubd_extrusions",
        "tosubd_pipes",
        "tosubd_conversion_edges",
    ];
    families.into_iter().flat_map(|family| {
        let path = root().join(format!("tools/rhino_oracle/observations/{family}.json"));
        serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap()).unwrap()["results"].as_array().unwrap().clone()
    }).filter(|r| !r["id"].as_str().unwrap().contains("tube_") && r["id"] != "tosubd_pipe_profile_frame_diagnostic").collect()
}
pub fn surface(v: &Value) -> NurbsSurface {
    let controls = v["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(
                coordinates::point(&p["point"]),
                p["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    let knots = |name: &str| {
        v[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect()
    };
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls,
        knots("knots_u"),
        knots("knots_v"),
    )
    .unwrap()
}
fn extrusion(definition: &Value) -> Brep {
    static SOURCES: std::sync::OnceLock<Vec<(Value, Brep)>> = std::sync::OnceLock::new();
    let sources = SOURCES.get_or_init(|| {
        rows()
            .iter()
            .filter_map(|row| {
                if row["value"]["source_files"]
                    .as_array()
                    .is_none_or(|f| f.is_empty())
                {
                    return None;
                }
                let path = root().join(format!(
                    "tools/rhino_oracle/fixtures/extrusions/{}.3dm",
                    row["id"].as_str().unwrap()
                ));
                let model = viboceros_io::read_3dm_file(path, Tolerance::DEFAULT).unwrap();
                let viboceros_io::ThreeDmGeometry::Brep(b) = &model.objects[0].geometry else {
                    panic!()
                };
                assert!(b.is_extrusion());
                Some((row["value"]["input"]["sources"][0].clone(), b.clone()))
            })
            .collect()
    });
    sources
        .iter()
        .find(|(d, _)| {
            d == definition
                || (*definition == json!({"kind":"extrusion"})
                    && *d == json!({"kind":"extrusion","profile":"rectangle","cap":true}))
        })
        .unwrap()
        .1
        .clone()
}
pub fn setup(v: &Value) -> Scene {
    let mut doc = Document::default();
    if let Some(t) = v["document_tolerance"].as_f64() {
        doc.set_tolerance(Tolerance::try_new(t, 1e-12, 1e-6).unwrap());
    }
    let layer = doc
        .add_layer("Conversion input", ColorRgb::new(0, 0, 255))
        .unwrap();
    doc.set_current_layer(layer).unwrap();
    let mut ids = Vec::new();
    for (i, row) in v["before"].as_array().unwrap().iter().enumerate() {
        let g = if v["input"]["sources"][i]["kind"] == "extrusion" {
            Geometry::Brep(extrusion(&v["input"]["sources"][i]))
        } else if v["input"]["sources"][i]["kind"] == "box" {
            Geometry::Brep(
                Brep::try_command_box(
                    viboceros_geometry::Frame3::try_from_directions(
                        viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
                        viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
                        viboceros_geometry::Vector3::try_new(0., 1., 0.).unwrap(),
                        doc.tolerance(),
                    )
                    .unwrap(),
                    [[0., 4.], [0., 3.], [0., 2.]],
                    doc.tolerance(),
                )
                .unwrap(),
            )
        } else if let Some(m) = row.get("mesh") {
            Geometry::Mesh(coordinates::mesh(m))
        } else if let Some(s) = row.get("surface") {
            Geometry::NurbsSurface(surface(s))
        } else {
            let faces = row["brep"].as_array().unwrap();
            if faces.len() == 1 {
                Geometry::NurbsSurface(surface(&faces[0]["surface"]))
            } else {
                Geometry::Brep(
                    Brep::try_disjoint_union(
                        faces
                            .iter()
                            .map(|f| {
                                Brep::try_surface_face(surface(&f["surface"]), doc.tolerance())
                                    .unwrap()
                            })
                            .collect(),
                        doc.tolerance(),
                    )
                    .unwrap(),
                )
            }
        };
        let id = doc.add_geometry(g).unwrap();
        doc.set_object_names([(id, Some(row["name"].as_str().unwrap().into()))])
            .unwrap();
        doc.set_objects_color([id], Some(ColorRgb::new(17, 43, 91)))
            .unwrap();
        ids.push(id);
    }
    doc.add_group(Some("Conversion assembly".into()), ids.iter().copied())
        .unwrap();
    let layer = doc
        .add_layer("Conversion current", ColorRgb::new(0, 128, 0))
        .unwrap();
    doc.set_current_layer(layer).unwrap();
    doc.select_objects_direct(
        ids.iter()
            .copied()
            .zip(v["before"].as_array().unwrap())
            .filter_map(|(id, r)| (r["selected"] == true).then_some(id)),
        SelectionMode::Replace,
    )
    .unwrap();
    doc.clear_history().unwrap();
    let originals = ids
        .iter()
        .map(|&id| doc.object(id).unwrap().geometry_snapshot().clone())
        .collect();
    Scene {
        doc,
        ids,
        originals,
    }
}
pub fn targets(scene: &Scene, v: &Value) -> Vec<ToSubDTarget> {
    v["input"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .flat_map(|(i, source)| match source["selected_faces"].as_array() {
            Some(faces) => faces
                .iter()
                .map(|f| ToSubDTarget::Face(scene.ids[i], f.as_u64().unwrap() as usize))
                .collect(),
            None => vec![ToSubDTarget::Whole(scene.ids[i])],
        })
        .collect()
}
pub fn options(v: &Value) -> ToSubDOptions {
    let o = &v["input"]["options"];
    ToSubDOptions {
        mesh_location: o["UseMesh"] == "Location",
        mesh_corners: o["MeshCorners"] == true,
        mesh_creases: o["MeshCreases"] == true,
        surface_method: if o["UseSurface"] == "ControlPoints" {
            SubDFromSurfaceMethod::ControlPoints
        } else {
            SubDFromSurfaceMethod::Location
        },
        surface_corners: o["SurfaceCorners"] == true,
        delete_input: o["DeleteInput"] == true,
    }
}
pub fn option_line(v: &Value) -> String {
    v["input"]["options"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, value)| {
            format!(
                "{k}={}",
                match value.as_bool() {
                    Some(true) => "Yes",
                    Some(false) => "No",
                    None => value.as_str().unwrap(),
                }
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn check(scene: &Scene, rows: &Value) -> Value {
    let expected = rows.as_array().unwrap();
    assert_eq!(scene.doc.objects().len(), expected.len());
    let mut checks = Vec::new();
    for (object, row) in scene.doc.objects().zip(expected) {
        if let Some(source) = row["source"].as_u64() {
            assert_eq!(object.id(), scene.ids[source as usize]);
        } else {
            assert!(!scene.ids.contains(&object.id()));
        }
        assert_eq!(object.attributes().name(), row["name"].as_str());
        assert_eq!(
            scene
                .doc
                .layer(object.attributes().layer_id())
                .unwrap()
                .name(),
            row["layer"].as_str().unwrap()
        );
        assert_eq!(
            scene.doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap()
        );
        assert_eq!(
            object
                .group_ids()
                .iter()
                .map(|&g| scene.doc.group(g).unwrap().name().unwrap())
                .collect::<Vec<_>>(),
            row["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g.as_str().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            object.attributes().object_color(),
            ColorRgb::new(17, 43, 91)
        );
        assert_eq!(
            object.attributes().color_source(),
            viboceros_document::ObjectColorSource::Object
        );
        if let Geometry::SubD(s) = object.geometry() {
            let mut controls = row["subd"].clone();
            controls.as_object_mut().unwrap().remove("surface_mesh");
            let mut check = subd_native::compare(s, &controls);
            subd_native::facets::certify_common_triangulation(s, &row["subd"], &mut check);
            checks.push(check);
        } else {
            let source = row["source"].as_u64().unwrap() as usize;
            assert_eq!(object.geometry_snapshot(), &scene.originals[source]);
        }
    }
    json!({"attributes_selection_and_identity":true,"objects":checks})
}

pub fn message(value: &Value) -> String {
    let line = value["history"]
        .as_str()
        .unwrap()
        .lines()
        .find(|s| s.starts_with("Created "))
        .unwrap();
    let word = line.split_whitespace().nth(1).unwrap();
    let count = if word == "no" {
        0
    } else {
        word.parse::<usize>().unwrap()
    };
    format!("Created {count} SubD(s)")
}
