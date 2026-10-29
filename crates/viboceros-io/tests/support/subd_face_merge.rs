use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use viboceros_geometry::{SubD, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

fn native_value(v: &Value) -> Value {
    let ids: Vec<u32> = serde_json::from_value(v["vertex_ids"].clone()).unwrap();
    let sorted = |id_key: &str, build: &dyn Fn(usize, u32) -> Value| {
        let mut rows = v[id_key]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, id)| {
                (
                    id.as_u64().unwrap() as u32,
                    build(i, id.as_u64().unwrap() as u32),
                )
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|(id, _)| *id);
        rows.into_iter().map(|(_, row)| row).collect::<Vec<_>>()
    };
    json!({
        "vertices": sorted("vertex_ids", &|i,id| json!([id,v["points"][i],v["vertex_tags"][i],v["surface_points"][i]])),
        "edges": sorted("edge_ids", &|i,id| json!([id,v["edges"][i].as_array().unwrap().iter().map(|j|ids[j.as_u64().unwrap() as usize]).collect::<Vec<_>>(),v["edge_tags"][i]])),
        "faces": sorted("face_ids", &|i,id| json!([id,v["faces"][i].as_array().unwrap().iter().map(|j|ids[j.as_u64().unwrap() as usize]).collect::<Vec<_>>()]))
    })
}
fn actual_value(s: &SubD) -> Value {
    json!({
        "vertices":s.vertex_ids().iter().enumerate().map(|(i,id)| json!([id,s.control_points()[i].to_array(),s.vertex_tags()[i],s.surface_points()[i].to_array()])).collect::<Vec<_>>(),
        "edges":s.edge_ids().iter().enumerate().map(|(i,id)|json!([id,s.edges()[i].map(|j|s.vertex_ids()[j as usize]),s.edge_tags()[i]])).collect::<Vec<_>>(),
        "faces":s.face_ids().iter().enumerate().map(|(i,id)|json!([id,s.faces()[i].iter().map(|&j|s.vertex_ids()[j as usize]).collect::<Vec<_>>()])).collect::<Vec<_>>()
    })
}
fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) if b.is_f64() => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-7,
                "{path}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert!(a.keys().eq(b.keys()), "{path}");
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
fn read(encoded: &Value) -> SubD {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        STANDARD.decode(encoded.as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let model = read_3dm_file(file.path(), Tolerance::DEFAULT).unwrap();
    let mut subds = model.objects.into_iter().filter_map(|o| match o.geometry {
        ThreeDmGeometry::SubD(s) => Some(s),
        _ => None,
    });
    let subd = subds.next().unwrap();
    assert!(subds.next().is_none());
    subd
}
fn records() -> (Value, Value) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut query = json!({"operations":[]});
    let mut result = json!({"results":[]});
    for (family, count) in [
        ("subd_merge_faces", 41),
        ("subd_merge_faces_holdouts", 32),
        ("subd_merge_faces_cleanup", 14),
        ("subd_merge_faces_valence", 7),
    ] {
        let q: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(q["operations"].as_array().unwrap().len(), count);
        assert_eq!(r["results"].as_array().unwrap().len(), count);
        query["operations"]
            .as_array_mut()
            .unwrap()
            .extend(q["operations"].as_array().unwrap().iter().cloned());
        result["results"]
            .as_array_mut()
            .unwrap()
            .extend(r["results"].as_array().unwrap().iter().cloned());
    }
    (query, result)
}
fn definition(snapshot: &Value) -> &Value {
    &snapshot
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row.get("subd").is_some())
        .unwrap()["subd"]
}

pub fn native_merge_archives_preserve_definition_ids_and_limit_points() {
    let (query, result) = records();
    assert_eq!(result["results"].as_array().unwrap().len(), 94);
    let mut sharpness = Vec::new();
    for (op, row) in query["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(result["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let before = read(&v["source_3dm"]);
        let after = read(&v["after_3dm"]);
        for (phase, s) in [("before", &before), ("after", &after)] {
            compare(
                &actual_value(s),
                &native_value(definition(&v[phase])),
                &format!("{name}/{phase}"),
            );
            let restored = SubD::try_from_archive(s.archive()).unwrap();
            assert_eq!(actual_value(&restored), actual_value(s), "{name}/{phase}");
            assert_eq!(
                restored.edge_sharpness(),
                s.edge_sharpness(),
                "{name}/{phase}"
            );
        }
        if !op["soft"].is_null() {
            let weight = op["soft"]["weight"].as_f64().unwrap();
            let expected =
                viboceros_opennurbs::subd_sharpness_from_percent(weight).unwrap() as f32 as f64;
            let pairs: Vec<[u32; 2]> = serde_json::from_value(op["soft"]["edges"].clone()).unwrap();
            for pair in pairs {
                let index = before
                    .edges()
                    .iter()
                    .position(|edge| {
                        let mut e = *edge;
                        e.sort();
                        e == pair
                    })
                    .unwrap();
                assert_eq!(before.edge_sharpness()[index], [expected; 2], "{name}");
            }
        }
        sharpness.push(json!({"id":name,"before_ids":before.edge_ids(),"before":before.edge_sharpness(),"after_ids":after.edge_ids(),"after":after.edge_sharpness()}));
    }
    if let Ok(path) = std::env::var("VIBOCEROS_SUBD_MERGE_SHARPNESS_AUDIT") {
        std::fs::write(path, serde_json::to_string_pretty(&sharpness).unwrap()).unwrap();
    }
}

pub fn face_merge_matches_native_topology_properties_and_component_identities() {
    use viboceros_geometry::SubDMergeComponent;
    let (_, result) = records();
    let mut failures = Vec::new();
    let mut report = Vec::new();
    for row in result["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap();
        let v = &row["value"];
        let source = read(&v["source_3dm"]);
        let expected = read(&v["after_3dm"]);
        let selection = v["selected_input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let id = c["id"].as_u64().unwrap() as u32;
                match c["kind"].as_str().unwrap() {
                    "face" => SubDMergeComponent::Face(
                        source.face_ids().iter().position(|&x| x == id).unwrap(),
                    ),
                    "vertex" => SubDMergeComponent::Vertex(
                        source.vertex_ids().iter().position(|&x| x == id).unwrap(),
                    ),
                    "edge" => SubDMergeComponent::Edge(
                        source.edges()[source.edge_ids().iter().position(|&x| x == id).unwrap()]
                            .map(|i| i as usize),
                    ),
                    _ => unreachable!(),
                }
            })
            .collect::<Vec<_>>();
        let original = source.clone();
        match source.merge_face_components(&selection) {
            Ok(merged) => {
                let mut differences = Vec::new();
                compare_limit_mesh(
                    &merged.subd,
                    &definition(&v["after"])["surface_mesh"],
                    name,
                    &mut differences,
                );
                differences_between(
                    &actual_value(&merged.subd),
                    &actual_value(&expected),
                    name,
                    &mut differences,
                );
                if merged.subd.edge_sharpness() != expected.edge_sharpness() {
                    differences.push(format!("{name}/sharpness"));
                }
                let native_selected = &v["after"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r.get("subd").is_some())
                    .unwrap()["selected_faces"];
                let loop_ids = |s: &SubD, id: u32| {
                    s.face_ids().iter().position(|&i| i == id).map(|i| {
                        s.faces()[i]
                            .iter()
                            .map(|&j| s.vertex_ids()[j as usize])
                            .collect::<Vec<_>>()
                    })
                };
                let mut expected_selected = native_selected
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as u32)
                    .filter(|&id| loop_ids(&source, id) != loop_ids(&expected, id))
                    .collect::<Vec<_>>();
                expected_selected.sort();
                let actual_selected = merged
                    .merged_faces
                    .iter()
                    .map(|&i| merged.subd.face_ids()[i])
                    .collect::<Vec<_>>();
                if actual_selected != expected_selected {
                    differences.push(format!(
                        "{name}/result faces: {actual_selected:?} != {expected_selected:?}"
                    ));
                }
                if merged.removed_faces != source.faces().len() - expected.faces().len() {
                    differences.push(format!("{name}/removed"));
                }
                report.push(json!({"id":name,"actual":actual_value(&merged.subd),"merged_faces":merged.merged_faces,"differences":differences}));
                failures.extend(differences);
            }
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                report.push(json!({"id":name,"error":e.to_string()}));
            }
        }
        assert_eq!(source, original, "immutable source: {name}");
    }
    if let Ok(path) = std::env::var("VIBOCEROS_SUBD_MERGE_AUDIT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} differences: {}",
        failures.len(),
        failures.join("\n")
    );
}

fn differences_between(
    actual: &Value,
    expected: &Value,
    path: &str,
    differences: &mut Vec<String>,
) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) if b.is_f64() => {
            if (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() > 1e-7 {
                differences.push(format!("{path}: {a} != {b}"));
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                differences_between(a, b, &format!("{path}/{i}"), differences);
            }
        }
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
            for (key, a) in a {
                differences_between(a, &b[key], &format!("{path}/{key}"), differences);
            }
        }
        _ => {
            if actual != expected {
                differences.push(format!("{path}: {actual} != {expected}"));
            }
        }
    }
}

fn compare_limit_mesh(s: &SubD, native: &Value, name: &str, differences: &mut Vec<String>) {
    let actual = match s.tessellate(2) {
        Ok(mesh) => mesh,
        Err(e) => {
            differences.push(format!("{name}/limit mesh: {e}"));
            return;
        }
    };
    let points: Vec<[f64; 3]> = serde_json::from_value(native["vertices"].clone()).unwrap();
    let mut remaining = actual
        .faces()
        .iter()
        .map(|face| {
            face.indices()
                .iter()
                .map(|&i| actual.vertices()[i as usize].to_array())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let faces = native["faces"].as_array().unwrap();
    if faces.len() != remaining.len() {
        differences.push(format!(
            "{name}/limit facet count: {} != {}",
            remaining.len(),
            faces.len()
        ));
        return;
    }
    for (i, face) in faces.iter().enumerate() {
        let expected = face
            .as_array()
            .unwrap()
            .iter()
            .map(|i| points[i.as_u64().unwrap() as usize])
            .collect::<Vec<_>>();
        let found = remaining.iter().position(|actual| {
            actual.len() == expected.len()
                && (0..actual.len()).any(|offset| {
                    actual.iter().enumerate().all(|(k, a)| {
                        a.iter()
                            .zip(expected[(k + offset) % expected.len()])
                            .all(|(&a, b)| (a - b).abs() <= 1e-7)
                    })
                })
        });
        if let Some(index) = found {
            remaining.swap_remove(index);
        } else {
            differences.push(format!("{name}/limit facet/{i}"));
        }
    }
}
