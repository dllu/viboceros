//! Native command scenes shared by command and frontend tests.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use viboceros_command::{
    ComponentSelectionKind as Kind,
    merge_faces::{Results, Target},
};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{SubD, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};
pub struct Fixture {
    pub document: Document,
    pub ids: BTreeMap<String, ObjectId>,
    pub targets: Vec<Target>,
    pub results: Results,
}
pub fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle").is_dir())
        .unwrap()
        .to_path_buf()
}
pub fn rows() -> Vec<Value> {
    let mut rows = Vec::new();
    for family in [
        "merge_faces_command",
        "merge_faces_command_more_v2",
        "merge_faces_command_order",
    ] {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root().join(format!("tools/rhino_oracle/observations/{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        rows.extend(data["results"].as_array().unwrap().iter().cloned());
    }
    rows
}
pub fn models(encoded: &Value) -> BTreeMap<String, (ObjectId, Geometry)> {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        STANDARD.decode(encoded.as_str().unwrap()).unwrap(),
    )
    .unwrap();
    read_3dm_file(file.path(), Tolerance::DEFAULT)
        .unwrap()
        .objects
        .into_iter()
        .map(|o| {
            let geometry = match o.geometry {
                ThreeDmGeometry::Mesh(m) => Geometry::Mesh(m),
                ThreeDmGeometry::SubD(s) => Geometry::SubD(s),
                ThreeDmGeometry::Point(p) => Geometry::Point(p),
                _ => panic!("unexpected owned geometry"),
            };
            (o.name.unwrap(), (ObjectId::from_uuid(o.id), geometry))
        })
        .collect()
}
pub fn fixture(row: &Value) -> Fixture {
    #[cfg(feature = "native-smlib")]
    {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let c = viboceros_geometry::NurbsCurve::try_clamped_uniform(
                2,
                vec![
                    viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap(),
                    viboceros_geometry::Point3::try_new(1., 1., 0.).unwrap(),
                    viboceros_geometry::Point3::try_new(2., 0., 0.).unwrap(),
                ],
            )
            .unwrap();
            // Command entry initializes the optional kernel in both the core
            // integration binary and the application binary using this helper.
            let mut doc = Document::default();
            let id = doc.add_geometry(Geometry::NurbsCurve(c)).unwrap();
            doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)
                .unwrap();
            viboceros_command::CommandRegistry::with_builtins()
                .execute(&mut doc, "ChangeDegree 3")
                .unwrap();
            let Geometry::NurbsCurve(c) = doc.object(id).unwrap().geometry() else {
                panic!("owned curve")
            };
            assert_eq!(c.degree(), 3);
        });
    }
    let value = &row["value"];
    let mut geometries = models(&value["source_3dm"]);
    let output = models(&value["after_3dm"]);
    for (name, (id, _)) in &geometries {
        assert_eq!(
            *id, output[name].0,
            "native MergeFaces preserves {name} UUID"
        );
    }
    let recipes: Value = serde_json::from_str(
        &std::fs::read_to_string(
            root().join("tools/rhino_oracle/fixtures/merge_faces_command_recipes.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let recipe = &recipes[row["id"].as_str().unwrap()];
    let mut document = Document::default();
    let mut ids = BTreeMap::new();
    let layer = document
        .add_layer("Merge inputs", ColorRgb::new(0, 0, 255))
        .unwrap();
    document.set_current_layer(layer).unwrap();
    let mut members = Vec::new();
    for before in value["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"].is_number())
    {
        let name = before["name"].as_str().unwrap();
        let (source_id, geometry) = geometries.remove(name).unwrap();
        let id = document
            .add_geometry_with_metadata_and_id(
                geometry,
                ObjectAttributes::on_layer(layer)
                    .with_name(name)
                    .with_object_color(ColorRgb::new(17, 43, 91)),
                BTreeMap::new(),
                Some(source_id),
            )
            .unwrap();
        ids.insert(name.into(), id);
        members.push(id);
    }
    if !members.is_empty() {
        document
            .add_group(Some("Merge input group".into()), members)
            .unwrap();
    }
    document.clear_selection();
    document.clear_history().unwrap();
    let (witness_id, witness_geometry) = geometries.remove("Undo witness").unwrap();
    let witness = document
        .add_geometry_with_metadata_and_id(
            witness_geometry,
            ObjectAttributes::on_layer(layer).with_name("Undo witness"),
            BTreeMap::new(),
            Some(witness_id),
        )
        .unwrap();
    ids.insert("Undo witness".into(), witness);
    document.clear_selection();
    let selected = value["before"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["selected"] == true)
        .map(|r| ids[r["name"].as_str().unwrap()])
        .collect::<Vec<_>>();
    document
        .select_objects_direct(selected, viboceros_document::SelectionMode::Replace)
        .unwrap();
    let mut targets = Vec::new();
    for (i, source) in recipe["sources"].as_array().unwrap().iter().enumerate() {
        let id = ids[&format!("Input {i}")];
        let geometry = document.object(id).unwrap().geometry();
        for pick in source["picks"].as_array().unwrap() {
            let (kind, index) = match (geometry, pick["kind"].as_str().unwrap()) {
                (_, "owner") => continue,
                (Geometry::Mesh(_), "face") => {
                    (Kind::MeshFace, pick["index"].as_u64().unwrap() as usize)
                }
                (Geometry::SubD(_), "face") => {
                    (Kind::SubDFace, pick["index"].as_u64().unwrap() as usize)
                }
                (Geometry::Mesh(m), "vertex") => {
                    let raw = pick["index"].as_u64().unwrap() as usize;
                    (
                        Kind::MeshVertex,
                        m.topology_vertex_points()
                            .iter()
                            .position(|&p| p == m.vertices()[raw])
                            .unwrap(),
                    )
                }
                (Geometry::SubD(_), "vertex") => {
                    (Kind::SubDVertex, pick["index"].as_u64().unwrap() as usize)
                }
                (Geometry::Mesh(m), "edge") => {
                    let ends: Vec<usize> =
                        serde_json::from_value(pick["vertices"].clone()).unwrap();
                    let a = m.vertices()[ends[0]];
                    let b = m.vertices()[ends[1]];
                    (
                        Kind::MeshEdge,
                        m.topology_edge_points()
                            .iter()
                            .position(|&e| e == [a, b] || e == [b, a])
                            .unwrap(),
                    )
                }
                (Geometry::SubD(s), "edge") => {
                    let ends: Vec<u32> = serde_json::from_value(pick["vertices"].clone()).unwrap();
                    (
                        Kind::SubDEdge,
                        s.edges()
                            .iter()
                            .position(|&e| e == [ends[0], ends[1]] || e == [ends[1], ends[0]])
                            .unwrap(),
                    )
                }
                _ => panic!("fixed owned pick"),
            };
            targets.push(Target {
                object: id,
                kind,
                index,
            });
        }
    }
    Fixture {
        document,
        ids,
        targets,
        results: Results::parse(recipe["results"].as_str().unwrap()).unwrap(),
    }
}
fn subd_actual(s: &SubD) -> Value {
    json!({"vertices":s.vertex_ids().iter().enumerate().map(|(i,id)|json!([id,s.control_points()[i].to_array(),s.vertex_tags()[i],s.surface_points()[i].to_array()])).collect::<Vec<_>>(),
        "edges":s.edge_ids().iter().enumerate().map(|(i,id)|json!([id,s.edges()[i].map(|i|s.vertex_ids()[i as usize]),s.edge_tags()[i]])).collect::<Vec<_>>(),
        "faces":s.face_ids().iter().enumerate().map(|(i,id)|json!([id,s.faces()[i].iter().map(|&i|s.vertex_ids()[i as usize]).collect::<Vec<_>>()])).collect::<Vec<_>>()})
}
fn subd_native(s: &Value) -> Value {
    let ids: Vec<u32> = serde_json::from_value(s["vertex_ids"].clone()).unwrap();
    let sorted = |key: &str, build: &dyn Fn(usize, u32) -> Value| {
        let mut v = s[key]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_u64().unwrap(), build(i, id.as_u64().unwrap() as u32)))
            .collect::<Vec<_>>();
        v.sort_by_key(|r| r.0);
        v.into_iter().map(|r| r.1).collect::<Vec<_>>()
    };
    json!({"vertices":sorted("vertex_ids",&|i,id|json!([id,s["points"][i],s["vertex_tags"][i],s["surface_points"][i]])),
        "edges":sorted("edge_ids",&|i,id|json!([id,s["edges"][i].as_array().unwrap().iter().map(|i|ids[i.as_u64().unwrap() as usize]).collect::<Vec<_>>(),s["edge_tags"][i]])),
        "faces":sorted("face_ids",&|i,id|json!([id,s["faces"][i].as_array().unwrap().iter().map(|i|ids[i.as_u64().unwrap() as usize]).collect::<Vec<_>>()]))})
}
pub fn geometry_value(g: &Geometry) -> Value {
    match g {
        Geometry::Point(p) => json!(p.to_array()),
        Geometry::SubD(s) => subd_actual(s),
        Geometry::Mesh(m) => {
            json!({"vertices":m.vertices().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":m.faces().iter().map(|f|f.indices().to_vec()).collect::<Vec<_>>(),
            "ngons":m.ngons().iter().map(|n|json!({"vertices":n.vertices(),"faces":n.faces()})).collect::<Vec<_>>(),"colors":m.vertex_colors().unwrap_or_default(),
            "uv":m.texture_coordinates().unwrap_or_default(),"normals":m.vertex_normals().unwrap_or_default().iter().map(|v|v.to_array()).collect::<Vec<_>>(),"valid":true})
        }
        _ => panic!("owned geometry"),
    }
}
pub fn component_value(doc: &Document, targets: &[Target]) -> Vec<Value> {
    let mut result = targets
        .iter()
        .map(|t| {
            let (kind, index) = match (t.kind, doc.object(t.object).unwrap().geometry()) {
                (Kind::MeshFace, _) => ("MeshFace", t.index),
                (Kind::MeshEdge, _) => ("MeshTopologyEdge", t.index),
                (Kind::MeshVertex, _) => ("MeshVertex", t.index),
                (Kind::SubDFace, Geometry::SubD(s)) => ("SubdFace", s.face_ids()[t.index] as usize),
                (Kind::SubDEdge, Geometry::SubD(s)) => ("SubdEdge", s.edge_ids()[t.index] as usize),
                (Kind::SubDVertex, Geometry::SubD(s)) => {
                    ("SubdVertex", s.vertex_ids()[t.index] as usize)
                }
                _ => panic!("owned target"),
            };
            json!({"kind":kind,"index":index})
        })
        .collect::<Vec<_>>();
    result.sort_by_key(|v| {
        (
            v["kind"].as_str().unwrap().to_owned(),
            v["index"].as_u64().unwrap(),
        )
    });
    result
}
pub fn snapshot(doc: &Document, targets: &[Target]) -> Value {
    json!(doc.objects().map(|o|{
        let a=o.attributes();let rgb=a.object_color();
        json!({"name":a.name().unwrap(),"object_id":o.id().to_string(),"kind":match o.geometry(){Geometry::Mesh(_)=>"mesh",Geometry::SubD(_)=>"subd",Geometry::Point(_)=>"point",_=>panic!()},
            "geometry":geometry_value(o.geometry()),"layer":doc.layer(a.layer_id()).unwrap().name(),
            "groups":o.group_ids().iter().map(|&id|doc.group(id).unwrap().name().to_owned()).collect::<Vec<_>>(),
            "color":[rgb.red,rgb.green,rgb.blue],"color_source":match a.color_source(){viboceros_document::ObjectColorSource::Layer=>0,viboceros_document::ObjectColorSource::Object=>1,viboceros_document::ObjectColorSource::Material=>2,viboceros_document::ObjectColorSource::Parent=>3},
            "selected":doc.is_selected(o.id()),"grips_on":doc.grip_mode(o.id()).is_some(),
            "components":component_value(doc,&targets.iter().filter(|t|t.object==o.id()).copied().collect::<Vec<_>>())})
    }).collect::<Vec<_>>())
}
fn expected(snapshot: &Value, ids: &BTreeMap<String, ObjectId>) -> Value {
    json!(
        snapshot
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let mut v = row.clone();
                v.as_object_mut().unwrap().remove("source");
                v.as_object_mut().unwrap().remove("runtime_serial");
                if row["object_id"].is_null() {
                    v["object_id"] = json!(ids[row["name"].as_str().unwrap()].to_string());
                }
                if row["kind"] == "subd" {
                    v["geometry"] = subd_native(&row["geometry"]);
                } else if row["kind"] == "mesh" {
                    // Public RhinoCommon colors use opacity; ON_Color uses transparency.
                    for color in v["geometry"]["colors"].as_array_mut().unwrap() {
                        color[3] = json!(255 - color[3].as_u64().unwrap());
                    }
                }
                v["components"].as_array_mut().unwrap().sort_by_key(|v| {
                    (
                        v["kind"].as_str().unwrap().to_owned(),
                        v["index"].as_u64().unwrap(),
                    )
                });
                v
            })
            .collect::<Vec<_>>()
    )
}
/// Native fixed-UUID repeats emit both insertion orders for changed meshes.
/// Only that contiguous NotSelected replacement run is compared by identity.
/// Selected ordering, unchanged objects, mesh/SubD order, Undo order, geometry
/// and every property stay strict.
pub fn compare_phase(
    doc: &Document,
    targets: &[Target],
    row: &Value,
    phase: &str,
    ids: &BTreeMap<String, ObjectId>,
) {
    let value = &row["value"];
    let mut actual = snapshot(doc, targets);
    let mut expected = expected(&value[phase], ids);
    if matches!(phase, "after" | "redone") && row["id"].as_str().unwrap().contains("NotSelected") {
        let changed = value["after"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| {
                a["kind"] == "mesh"
                    && value["before"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|b| b["name"] == a["name"] && b["geometry"] != a["geometry"])
            })
            .map(|r| {
                r["object_id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| ids[r["name"].as_str().unwrap()].to_string())
            })
            .collect::<std::collections::BTreeSet<_>>();
        let positions = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .enumerate()
                .filter(|(_, r)| changed.contains(r["object_id"].as_str().unwrap()))
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };
        let slots = positions(&actual);
        assert_eq!(
            slots,
            positions(&expected),
            "{} {phase}: replacement run",
            row["id"]
        );
        assert!(slots.windows(2).all(|w| w[1] == w[0] + 1));
        for v in [&mut actual, &mut expected] {
            let mut records = slots.iter().map(|&i| v[i].clone()).collect::<Vec<_>>();
            records.sort_by_key(|r| r["object_id"].as_str().unwrap().to_owned());
            for (&i, r) in slots.iter().zip(records) {
                v[i] = r;
            }
        }
    }
    compare(
        &actual,
        &expected,
        &format!("{}/{phase}", row["id"].as_str().unwrap()),
    );
}
pub fn compare(actual: &Value, expected: &Value, name: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) if b.is_f64() => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-7,
            "{name}: {a} != {b}"
        ),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{name}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{name}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert!(a.keys().eq(b.keys()), "{name}: keys");
            for (k, a) in a {
                compare(a, &b[k], &format!("{name}/{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{name}"),
    }
}
