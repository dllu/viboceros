//! Independent native source files, component graphs and complete scene history.
#![allow(dead_code)]
use super::{mesh_native, scene, subd_native};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{SubD, Tolerance};

pub fn rows() -> Vec<(&'static str, Value)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    let mut rows = Vec::new();
    for (family, count) in [
        ("subd_cap_regions", 18),
        ("subd_cap_holdouts", 8),
        ("subd_cap_annuli", 20),
        ("subd_cap_metric", 12),
    ] {
        let capture: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(capture["results"].as_array().unwrap().len(), count);
        for row in capture["results"].as_array().unwrap() {
            assert_eq!(row["value"]["success"], true);
            rows.push((family, row.clone()));
        }
    }
    rows
}

type SampleCache = std::collections::BTreeMap<(Vec<u8>, Vec<u8>), f64>;
thread_local! { static SAMPLE_ERRORS: std::cell::RefCell<SampleCache> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) }; }

fn sampled_error(actual: &SubD, native: &Value) -> f64 {
    // History restores identical immutable archives. Cache only identical input
    // bytes and full target mesh values; geometry, tags and attributes are still
    // checked independently at every lifecycle stage.
    let key = (
        actual.archive().to_vec(),
        serde_json::to_vec(&native["surface_mesh"]).unwrap(),
    );
    if let Some(value) = SAMPLE_ERRORS.with(|c| c.borrow().get(&key).copied()) {
        return value;
    }
    let mesh = actual.tessellate(2).unwrap();
    let target = mesh_native::mesh(&native["surface_mesh"]);
    let error = subd_native::directed(&mesh, &target).max(subd_native::directed(&target, &mesh));
    SAMPLE_ERRORS.with(|c| c.borrow_mut().insert(key, error));
    error
}

pub fn setup(row: &Value, family: &str) -> (Document, Vec<ObjectId>) {
    let (doc, ids) = scene::setup_from_family_with_tolerance(
        row,
        family,
        Some(Tolerance::try_new(1e-9, 1e-12, 1e-10).unwrap()),
    );
    for (index, &id) in ids.iter().enumerate() {
        let Geometry::SubD(subd) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        let native = &row["value"]["before"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == index)
            .unwrap()["subd"];
        assert_eq!(
            json!(
                subd.control_points()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>()
            ),
            native["points"]
        );
        assert_eq!(json!(subd.faces()), native["faces"]);
        assert_eq!(json!(subd.vertex_ids()), native["vertex_ids"]);
        assert_eq!(json!(subd.edge_ids()), native["edge_ids"]);
        assert_eq!(json!(subd.face_ids()), native["face_ids"]);
    }
    (doc, ids)
}

pub fn command(value: &Value) -> String {
    format!(
        "Cap {}",
        value["input"]["command_arguments"]
            .as_str()
            .unwrap_or_default()
            .replace('_', "")
    )
}

fn cyclic<T: Ord + Clone>(face: &[T]) -> Vec<T> {
    (0..face.len())
        .map(|i| {
            face[i..]
                .iter()
                .chain(&face[..i])
                .cloned()
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap()
}

fn decoded(native: &Value) -> SubD {
    let file = tempfile::Builder::new().suffix(".3dm").tempfile().unwrap();
    std::fs::write(
        file.path(),
        STANDARD
            .decode(native["geometry_3dm"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    let (doc, _, _, _) =
        viboceros_command::open_3dm_with_views(file.path().to_str().unwrap()).unwrap();
    let Geometry::SubD(subd) = doc.objects().next().unwrap().geometry() else {
        panic!("native SubD archive")
    };
    subd.clone()
}

pub fn geometry(actual: &SubD, native: &Value) -> Value {
    assert_eq!(native["valid"], true);
    let expected = decoded(native);
    // These authored inputs have unique control positions. Component IDs, not
    // nearest positions, establish correspondence for all original components.
    let map = actual
        .control_points()
        .iter()
        .map(|&p| {
            expected
                .control_points()
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    p.distance_to(**a)
                        .unwrap()
                        .total_cmp(&p.distance_to(**b).unwrap())
                })
                .unwrap()
                .0
        })
        .collect::<Vec<_>>();
    let bijection = actual.control_points().len() == expected.control_points().len()
        && map
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == map.len();
    let mut control_error = 0f64;
    let mut limit_error = 0f64;
    for (i, &j) in map.iter().enumerate() {
        control_error = control_error.max(
            actual.control_points()[i]
                .distance_to(expected.control_points()[j])
                .unwrap(),
        );
        limit_error = limit_error.max(
            actual.surface_points()[i]
                .distance_to(expected.surface_points()[j])
                .unwrap(),
        );
    }
    let mut a_faces = actual
        .faces()
        .iter()
        .map(|f| cyclic(&f.iter().map(|&i| map[i as usize]).collect::<Vec<_>>()))
        .collect::<Vec<_>>();
    let mut b_faces = expected
        .faces()
        .iter()
        .map(|f| cyclic(&f.iter().map(|&i| i as usize).collect::<Vec<_>>()))
        .collect::<Vec<_>>();
    a_faces.sort();
    b_faces.sort();
    let mut a_edges = actual
        .edges()
        .iter()
        .zip(actual.edge_tags())
        .map(|(e, &t)| {
            let (a, b) = (map[e[0] as usize], map[e[1] as usize]);
            (a.min(b), a.max(b), t)
        })
        .collect::<Vec<_>>();
    let mut b_edges = expected
        .edges()
        .iter()
        .zip(expected.edge_tags())
        .map(|(e, &t)| {
            let (a, b) = (e[0] as usize, e[1] as usize);
            (a.min(b), a.max(b), t)
        })
        .collect::<Vec<_>>();
    a_edges.sort();
    b_edges.sort();
    let vertex_tags = bijection
        && map
            .iter()
            .enumerate()
            .all(|(i, &j)| actual.vertex_tags()[i] == expected.vertex_tags()[j]);
    let sample_error = sampled_error(actual, native);
    let property_rows = |subd: &SubD, permutation: &[usize]| {
        let mut rows = subd
            .faces()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let corners = f
                    .iter()
                    .enumerate()
                    .map(|(k, &v)| {
                        (
                            permutation[v as usize],
                            subd.face_texture_points()[i]
                                .as_ref()
                                .and_then(|p| p[k])
                                .map(|p| p.map(f64::to_bits)),
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    cyclic(&corners),
                    subd.face_properties()[i].color,
                    subd.face_properties()[i].material_channel,
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    };
    let properties = (a_faces == b_faces).then(|| {
        actual.texture_coordinate_type() == expected.texture_coordinate_type()
            && property_rows(actual, &map)
                == property_rows(
                    &expected,
                    &(0..expected.control_points().len()).collect::<Vec<_>>(),
                )
    });
    json!({"counts":actual.control_points().len()==expected.control_points().len() && actual.edges().len()==expected.edges().len() && actual.faces().len()==expected.faces().len(),
        "controls":bijection && control_error<=1e-6,"control_error":control_error,"limit_points":bijection && limit_error<=1e-6,"limit_error":limit_error,
        "faces":a_faces==b_faces,"edge_tags":a_edges==b_edges,"vertex_tags":vertex_tags,"properties":properties,
        "sample_surface":sample_error<=1e-6,"sample_error":sample_error})
}

pub fn compare(doc: &Document, ids: &[ObjectId], rows: &Value) -> Value {
    assert_eq!(doc.objects().len(), rows.as_array().unwrap().len());
    let mut checks = Vec::new();
    for row in rows.as_array().unwrap() {
        let object = scene::object_for_row(doc, ids, row);
        assert!(
            scene::metadata_matches(doc, object, row, true),
            "scene metadata {}",
            row["name"]
        );
        match object.geometry() {
            Geometry::SubD(subd) => checks.push(geometry(subd, &row["subd"])),
            Geometry::Point(p) => assert_eq!(*p, scene::point(&row["point"])),
            _ => panic!("unexpected scene geometry"),
        }
    }
    json!(checks)
}

pub fn assert_diagnostics(diagnostics: &[Value]) {
    assert_eq!(diagnostics.len(), 58);
    let legacy: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/subd_cap_retained_gaps.json"
    ))
    .unwrap();
    let extra: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/subd_cap_world_retained_gaps.json"
    ))
    .unwrap();
    for phase in ["after", "after_script", "redo"] {
        for field in [
            "counts",
            "controls",
            "limit_points",
            "faces",
            "edge_tags",
            "vertex_tags",
            "properties",
            "sample_surface",
            "uncompared_properties",
        ] {
            let mut expected = Vec::new();
            for source in [&legacy, &extra] {
                if let Some(rows) = source[field].as_array() {
                    expected.extend(rows.iter().cloned());
                }
            }
            let actual = diagnostics
                .iter()
                .filter(|r| {
                    r[phase].as_array().unwrap().iter().any(|o| {
                        if field == "uncompared_properties" {
                            o["properties"].is_null()
                        } else {
                            o[field] == false
                        }
                    })
                })
                .map(|r| r["id"].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                json!(actual),
                json!(expected),
                "unexplained SubD Cap change {phase}/{field}"
            );
        }
    }
    for row in diagnostics {
        for phase in ["before", "after", "after_script", "undo", "redo"] {
            for object in row[phase].as_array().unwrap() {
                for field in [
                    "counts",
                    "controls",
                    "limit_points",
                    "faces",
                    "edge_tags",
                    "vertex_tags",
                    "properties",
                    "sample_surface",
                ] {
                    if ["before", "undo"].contains(&phase) {
                        assert_eq!(
                            object[field], true,
                            "{}/{phase}/{field}: {object}",
                            row["id"]
                        );
                    } else {
                        assert!(
                            object[field].is_boolean()
                                || (field == "properties" && object[field].is_null()),
                            "invalid comparison result"
                        );
                    }
                }
            }
        }
    }
}
