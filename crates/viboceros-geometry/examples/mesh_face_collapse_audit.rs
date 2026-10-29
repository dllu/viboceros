//! Audit frozen native meshes and explicitly named public OpenNURBS operations.
//! Storage agreement is a diagnostic, not a native algorithm or image proof.
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use viboceros_geometry::{MeshFace, Point3, Tolerance, TriangleMesh};
use viboceros_opennurbs::{MeshData, Ngon, dissolve_mesh_vertices};

fn decode(value: &Value) -> MeshData {
    let vertices = serde_json::from_value(value["vertices"].clone()).unwrap();
    let polygons: Vec<Vec<u32>> = serde_json::from_value(value["faces"].clone()).unwrap();
    let channel = |name: &str| {
        let array = value[name].as_array().unwrap();
        (!array.is_empty()).then(|| value[name].clone())
    };
    MeshData {
        vertices,
        faces: polygons
            .iter()
            .map(|f| [f[0], f[1], f[2], *f.get(3).unwrap_or(&f[2])])
            .collect(),
        colors: channel("colors").map(|v| serde_json::from_value(v).unwrap()),
        texture_coordinates: channel("uv").map(|v| serde_json::from_value(v).unwrap()),
        vertex_normals: channel("normals").map(|v| serde_json::from_value(v).unwrap()),
        ngons: value["ngons"]
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

fn validate(data: &MeshData) -> Value {
    let result = TriangleMesh::try_new_faces(
        data.vertices
            .iter()
            .copied()
            .map(Point3::try_from)
            .collect::<Result<_, _>>()
            .unwrap(),
        data.faces
            .iter()
            .map(|&[a, b, c, d]| {
                if c == d {
                    MeshFace::Triangle([a, b, c])
                } else {
                    MeshFace::Quad([a, b, c, d])
                }
            })
            .collect(),
        Tolerance::MESH_VALIDATION,
    );
    match result {
        Ok(mesh) => {
            json!({"valid_facets":true, "manifold":mesh.topology().is_manifold(), "closed":mesh.topology().is_closed()})
        }
        Err(e) => json!({"valid_facets":false,"error":e.to_string()}),
    }
}

fn trial(data: &MeshData, selected: &[usize], expected: &MeshData) -> Value {
    match dissolve_mesh_vertices(data, selected) {
        Ok(actual) => json!({
            "selected_raw_vertices":selected,
            "complete_storage_equal":actual == *expected,
            "positions_equal":actual.vertices == expected.vertices,
            "faces_equal":actual.faces == expected.faces,
            "colors_equal":actual.colors == expected.colors,
            "uv_equal":actual.texture_coordinates == expected.texture_coordinates,
            "normals_equal":actual.vertex_normals == expected.vertex_normals,
            "ngons_equal":actual.ngons == expected.ngons,
            "vertices":actual.vertices,"faces":actual.faces,
            "validation":validate(&actual),
        }),
        Err(e) => json!({"selected_raw_vertices":selected,"error":e.to_string()}),
    }
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut rows = Vec::new();
    let mut families = std::env::args().skip(1).collect::<Vec<_>>();
    if families.is_empty() {
        families = fs::read_dir(root.join("observations"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter_map(|name| {
                name.strip_suffix(".json")
                    .filter(|name| name.starts_with("mesh_face_collapse"))
                    .map(str::to_owned)
            })
            .collect();
        families.sort();
    }
    assert!(!families.is_empty(), "no mesh face collapse families found");
    for family in families {
        let query: Value = serde_json::from_slice(
            &fs::read(root.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native: Value = serde_json::from_slice(
            &fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        let ops = query["operations"].as_array().unwrap();
        let outputs = native["results"].as_array().unwrap();
        assert_eq!(ops.len(), outputs.len());
        for (op, row) in ops.iter().zip(outputs) {
            assert_eq!(op["id"], row["id"]);
            let before = decode(&row["value"]["before"]);
            let after = decode(&row["value"]["after"]);
            let face = before.faces[op["face"].as_u64().unwrap() as usize];
            let corners = if face[2] == face[3] {
                &face[..3]
            } else {
                &face[..]
            };
            let corners = corners.iter().map(|&i| i as usize).collect::<Vec<_>>();
            rows.push(json!({"family":family,"id":op["id"],
                "before":validate(&before),"after":validate(&after),
                "native_geometry_changed":before.vertices != after.vertices || before.faces != after.faces,
                "dissolve_first_raw_corner":trial(&before,&corners[..1],&after),
                "dissolve_all_raw_corners":trial(&before,&corners,&after),
            }));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"public_operation":"OpenNURBS DissolveOrDelete(raw vertices)","rows":rows})
        )
        .unwrap()
    );
}
