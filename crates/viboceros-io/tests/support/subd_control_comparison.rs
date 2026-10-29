use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use viboceros_geometry::{SubD, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

pub(crate) fn native_value(v: &Value) -> Value {
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
pub(crate) fn actual_value(s: &SubD) -> Value {
    json!({
        "vertices":s.vertex_ids().iter().enumerate().map(|(i,id)| json!([id,s.control_points()[i].to_array(),s.vertex_tags()[i],s.surface_points()[i].to_array()])).collect::<Vec<_>>(),
        "edges":s.edge_ids().iter().enumerate().map(|(i,id)|json!([id,s.edges()[i].map(|j|s.vertex_ids()[j as usize]),s.edge_tags()[i]])).collect::<Vec<_>>(),
        "faces":s.face_ids().iter().enumerate().map(|(i,id)|json!([id,s.faces()[i].iter().map(|&j|s.vertex_ids()[j as usize]).collect::<Vec<_>>()])).collect::<Vec<_>>()
    })
}
pub(crate) fn compare(actual: &Value, expected: &Value, path: &str) {
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
pub(crate) fn read(encoded: &Value) -> SubD {
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
pub(crate) fn differences_between(
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

pub(crate) fn compare_limit_mesh(
    s: &SubD,
    native: &Value,
    name: &str,
    differences: &mut Vec<String>,
) {
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
