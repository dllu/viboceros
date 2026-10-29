use serde_json::{Value, json};
use viboceros_geometry::{
    MeshFace, MeshMergeComponent, MeshNgon, Point3, Tolerance, TriangleMesh, Vector3,
};
use viboceros_opennurbs::{MeshData, Ngon};

fn data(v: &Value) -> MeshData {
    let colors: Vec<[u8; 4]> = serde_json::from_value::<Vec<[u8; 4]>>(v["colors"].clone())
        .unwrap()
        .into_iter()
        .map(|mut color| {
            color[3] = 255 - color[3];
            color
        })
        .collect();
    let uv: Vec<[f64; 2]> = serde_json::from_value(v["uv"].clone()).unwrap();
    let normals: Vec<[f64; 3]> = serde_json::from_value(v["normals"].clone()).unwrap();
    MeshData {
        vertices: serde_json::from_value(v["vertices"].clone()).unwrap(),
        faces: v["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                let p: Vec<u32> = serde_json::from_value(f.clone()).unwrap();
                [p[0], p[1], p[2], *p.get(3).unwrap_or(&p[2])]
            })
            .collect(),
        colors: (!colors.is_empty()).then_some(colors),
        texture_coordinates: (!uv.is_empty()).then_some(uv),
        vertex_normals: (!normals.is_empty()).then_some(normals),
        ngons: v["ngons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| Ngon {
                vertices: serde_json::from_value(g["vertices"].clone()).unwrap(),
                faces: serde_json::from_value(g["faces"].clone()).unwrap(),
            })
            .collect(),
    }
}
fn from_data(d: MeshData) -> TriangleMesh {
    TriangleMesh::try_new_faces(
        d.vertices
            .into_iter()
            .map(|p| Point3::try_new(p[0], p[1], p[2]).unwrap())
            .collect(),
        d.faces
            .into_iter()
            .map(|f| {
                if f[2] == f[3] {
                    MeshFace::Triangle([f[0], f[1], f[2]])
                } else {
                    MeshFace::Quad(f)
                }
            })
            .collect(),
        Tolerance::MESH_VALIDATION,
    )
    .unwrap()
    .try_with_vertex_colors(d.colors)
    .unwrap()
    .try_with_texture_coordinates(d.texture_coordinates)
    .unwrap()
    .try_with_vertex_normals(d.vertex_normals.map(|normals| {
        normals
            .into_iter()
            .map(|n| Vector3::try_new(n[0], n[1], n[2]).unwrap())
            .collect()
    }))
    .unwrap()
    .try_with_ngons(
        d.ngons
            .into_iter()
            .map(|g| MeshNgon::from_parts(g.vertices, g.faces))
            .collect(),
    )
    .unwrap()
}

fn value(m: &TriangleMesh) -> Value {
    let d = MeshData {
        vertices: m.vertices().iter().map(|p| p.to_array()).collect(),
        faces: m
            .faces()
            .iter()
            .map(|f| {
                let p = f.indices();
                [p[0], p[1], p[2], *p.get(3).unwrap_or(&p[2])]
            })
            .collect(),
        colors: m.vertex_colors().map(<[_]>::to_vec),
        texture_coordinates: m.texture_coordinates().map(<[_]>::to_vec),
        vertex_normals: m
            .vertex_normals()
            .map(|normals| normals.iter().map(|n| n.to_array()).collect()),
        ngons: m
            .ngons()
            .iter()
            .map(|g| Ngon {
                vertices: g.vertices().to_vec(),
                faces: g.faces().to_vec(),
            })
            .collect(),
    };
    json!({"vertices":d.vertices,"faces":d.faces.iter().map(|f|if f[2]==f[3] {f[..3].to_vec()}else{f.to_vec()}).collect::<Vec<_>>(),
        "colors":d.colors.unwrap_or_default().into_iter().map(|mut color|{color[3]=255-color[3];color}).collect::<Vec<_>>(),"uv":d.texture_coordinates.unwrap_or_default(),"normals":d.vertex_normals.unwrap_or_default(),
        "ngons":d.ngons.iter().map(|g|json!({"vertices":g.vertices,"faces":g.faces})).collect::<Vec<_>>(),"valid":true})
}
fn compare(a: &Value, b: &Value, path: &str, diffs: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) if b.is_f64() => {
            if (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() > 1e-7 {
                diffs.push(path.into());
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"), diffs);
            }
        }
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
            for (k, a) in a {
                compare(a, &b[k], &format!("{path}/{k}"), diffs);
            }
        }
        _ => {
            if a != b {
                diffs.push(path.into());
            }
        }
    }
}
pub fn audit_all_native_mesh_storage_and_channels() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut failures = Vec::new();
    let mut report = Vec::new();
    for (family, count) in [
        ("mesh_merge_faces", 36),
        ("mesh_merge_faces_boundaries", 12),
        ("mesh_merge_faces_components", 26),
    ] {
        let q: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let r: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(r["results"].as_array().unwrap().len(), count);
        for (op, row) in q["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(r["results"].as_array().unwrap())
        {
            assert_eq!(op["id"], row["id"]);
            let name = row["id"].as_str().unwrap();
            let native = &row["value"];
            let source = data(&native["before"]);
            let mesh = from_data(source.clone());
            let before = mesh.clone();
            let selected = op["components"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| match c["kind"].as_str().unwrap() {
                    "face" => MeshMergeComponent::Face(c["index"].as_u64().unwrap() as usize),
                    "vertex" => MeshMergeComponent::Vertex(c["index"].as_u64().unwrap() as usize),
                    "edge" => MeshMergeComponent::Edge([
                        c["vertices"][0].as_u64().unwrap() as usize,
                        c["vertices"][1].as_u64().unwrap() as usize,
                    ]),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>();
            let result = mesh.merge_face_components_mapped(&selected);
            assert_eq!(mesh, before, "source changed: {name}");
            let mut diffs = Vec::new();
            compare(&value(&mesh), &native["before"], "before", &mut diffs);
            let actual = match result {
                Ok(result) => {
                    let merged = result.mesh;
                    let removed = result.removed_faces;
                    let expected_faces = native["selected_after"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|c| c["kind"] == "MeshFace")
                        .map(|c| c["index"].as_u64().unwrap() as usize)
                        .collect::<Vec<_>>();
                    if result.merged_faces != expected_faces {
                        diffs.push(format!(
                            "selected: {:?} != {expected_faces:?}",
                            result.merged_faces
                        ));
                    }
                    let actual = value(&merged);
                    compare(&actual, &native["after"], "after", &mut diffs);
                    let facets = match merged.validate_face_geometry(Tolerance::MESH_VALIDATION) {
                        Ok(()) => json!({"valid":true}),
                        Err(error) => json!({"valid":false,"error":error.to_string()}),
                    };
                    json!({"mesh":actual,"removed_logical_faces":removed,"fixed_triangle_validation":facets})
                }
                Err(e) => {
                    diffs.push(format!("kernel rejected output: {e}"));
                    json!({"error":e.to_string()})
                }
            };
            if !diffs.is_empty() {
                failures.push((name.to_string(), diffs.clone()));
            }
            if name == "ring-none" {
                let (coplanar, removed) = mesh.merge_coplanar_faces(Tolerance::DEFAULT).unwrap();
                assert_eq!(removed, 7, "coplanar policy was not restored");
                assert_eq!(coplanar.ngons().len(), 1);
                assert_eq!(coplanar.ngons()[0].faces().len(), 8);
            }
            report.push(json!({"id":name,"actual":actual,"differences":diffs}));
        }
    }
    assert_eq!(report.len(), 74);
    if let Ok(path) = std::env::var("VIBOCEROS_MESH_MERGE_AUDIT_OUTPUT") {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    assert!(
        failures.is_empty(),
        "MergeFaces complete storage differences: {failures:?}"
    );
}
