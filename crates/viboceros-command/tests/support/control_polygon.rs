//! Full command replay from immutable native inputs, including metadata/history.
use serde_json::{Value, json};
use std::path::Path;
use viboceros_document::{Document, Geometry, Object, ObjectId, SelectionMode};

fn near<const N: usize>(points: &[[f64; N]], expected: &Value, context: &str) -> f64 {
    let expected: Vec<[f64; N]> = expected
        .as_array()
        .unwrap()
        .iter()
        .map(|p| std::array::from_fn(|i| p[i].as_f64().unwrap()))
        .collect();
    assert_eq!(points.len(), expected.len(), "{context}");
    let error = points
        .iter()
        .zip(&expected)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()))
        .fold(0., f64::max);
    assert!(error <= 1e-6, "{context}: {error}");
    error
}

pub fn compare(document: &Document, sources: &[ObjectId], expected: &Value, context: &str) -> f64 {
    let objects = document.objects().collect::<Vec<_>>();
    let expected = expected.as_array().unwrap();
    assert_eq!(objects.len(), expected.len(), "{context}");
    let mut error = 0_f64;
    for (object, native) in objects.into_iter().zip(expected) {
        let attrs = object.attributes();
        assert_eq!(attrs.name(), native["name"].as_str(), "{context}");
        assert_eq!(
            document.layer(attrs.layer_id()).unwrap().name(),
            native["layer"].as_str().unwrap(),
            "{context}"
        );
        let color = attrs.object_color();
        assert_eq!(
            json!([color.red, color.green, color.blue]),
            native["color"],
            "{context}"
        );
        assert_eq!(
            attrs.color_source() as u8,
            native["color_source"].as_u64().unwrap() as u8,
            "{context}"
        );
        let groups = object
            .group_ids()
            .iter()
            .map(|id| document.group(*id).unwrap().name().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(json!(groups), native["groups"], "{context}");
        assert_eq!(
            document.is_selected(object.id()),
            native["selected"].as_bool().unwrap(),
            "{context} {}",
            native["name"]
        );
        if let Some(index) = native["source"].as_u64() {
            assert_eq!(object.id(), sources[index as usize]);
            continue;
        }
        match object.geometry() {
            Geometry::Mesh(mesh) => {
                let n = &native["mesh"];
                error = error.max(near(
                    &mesh
                        .vertices()
                        .iter()
                        .map(|p| p.to_array())
                        .collect::<Vec<_>>(),
                    &n["vertices"],
                    context,
                ));
                assert_eq!(
                    json!(mesh.faces().iter().map(|f| f.indices()).collect::<Vec<_>>()),
                    n["faces"],
                    "{context}"
                );
                assert_eq!(
                    json!(
                        mesh.ngons()
                            .iter()
                            .map(|g| json!({"vertices":g.vertices(),"faces":g.faces()}))
                            .collect::<Vec<_>>()
                    ),
                    n["ngons"],
                    "{context}"
                );
                error = error.max(near(
                    &mesh
                        .vertex_normals()
                        .unwrap_or_default()
                        .iter()
                        .map(|n| n.to_array())
                        .collect::<Vec<_>>(),
                    &n["normals"],
                    context,
                ));
                if let Some(uv) = n.get("texture_coordinates") {
                    error = error.max(near(
                        mesh.texture_coordinates().unwrap_or_default(),
                        uv,
                        context,
                    ));
                }
            }
            Geometry::Polyline(line) => {
                error = error.max(near(
                    &line
                        .vertices()
                        .iter()
                        .map(|p| p.to_array())
                        .collect::<Vec<_>>(),
                    &native["polyline"],
                    context,
                ));
            }
            other => panic!("{context}: unexpected output {other:?}"),
        }
    }
    error
}

pub fn setup(family: &str, id: &str, v: &Value, context: &str) -> (Document, Vec<ObjectId>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/fixtures").is_dir())
        .unwrap();
    let path = root.join(format!(
        "tools/rhino_oracle/fixtures/{family}/source/{id}.3dm"
    ));
    let (mut document, _, _, _) =
        viboceros_command::open_3dm_with_views(path.to_str().unwrap()).unwrap();
    let sources = document.objects().map(Object::id).collect::<Vec<_>>();
    let before = v["before"].as_array().unwrap();
    assert_eq!(sources.len(), before.len());
    let current = document
        .layer_by_name("Control polygon current")
        .unwrap()
        .id();
    document.set_current_layer(current).unwrap();
    let mut groups = std::collections::BTreeMap::<String, Vec<ObjectId>>::new();
    for (&source, native) in sources.iter().zip(before) {
        for name in native["groups"].as_array().unwrap() {
            groups
                .entry(name.as_str().unwrap().to_owned())
                .or_default()
                .push(source);
        }
    }
    for (name, members) in groups {
        document.add_group(Some(name), members).unwrap();
    }
    document.clear_selection();
    let selected = sources
        .iter()
        .zip(before)
        .filter(|(_, v)| v["selected"] == true)
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    document
        .select_objects_direct(selected, SelectionMode::Replace)
        .unwrap();
    compare(&document, &sources, &v["before"], context);

    document.clear_history().unwrap();
    (document, sources)
}
