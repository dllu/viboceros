use serde_json::{Value, json};
use viboceros_geometry::{MeshFace, TriangleMesh};
#[path = "support/mesh_conversion.rs"]
mod raw_mesh;

fn flips(source: &TriangleMesh, result: &TriangleMesh) -> Vec<usize> {
    assert_eq!(source.vertices(), result.vertices());
    assert_eq!(source.face_count(), result.face_count());
    source
        .faces()
        .iter()
        .zip(result.faces())
        .enumerate()
        .filter_map(|(i, (&a, &b))| {
            if a == b {
                None
            } else {
                let reversed = match a {
                    MeshFace::Triangle([a, b, c]) => MeshFace::Triangle([a, c, b]),
                    MeshFace::Quad([a, b, c, d]) => MeshFace::Quad([a, d, c, b]),
                };
                assert_eq!(reversed, b);
                Some(i)
            }
        })
        .collect()
}

#[test]
fn sdk_unification_matches_native_counts_winding_channels_and_readonly_queries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    let mut failures = Vec::new();
    for family in [
        "sdk",
        "extra",
        "holdouts",
        "seeds",
        "followups",
        "walks",
        "components",
        "runtime",
    ] {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("unify_normals_{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["input"]["command"].is_string() {
                continue;
            }
            let id = row["id"].as_str().unwrap();
            eprintln!("Unify SDK {id}");
            assert_eq!(v["before"], v["after_count"]);
            let (source, invalid) = raw_mesh::mesh(&v["before"], false);
            assert!(invalid.is_empty());
            let (expected, invalid) = raw_mesh::mesh(&v["after"], false);
            assert!(invalid.is_empty());
            let before = source.clone();
            let predicted = source.normal_unification_count();
            let (actual, changed) = source.unified_normals().unwrap();
            let first_flips = flips(&source, &actual);
            assert_eq!(changed, first_flips.len());
            assert_eq!(predicted, changed);
            assert_eq!(source, before);
            let (again, second) = actual.unified_normals().unwrap();
            let repeat_flips = flips(&actual, &again);
            assert_eq!(second, repeat_flips.len());
            let (target, invalid) = raw_mesh::mesh(&v["after_second"], false);
            assert!(invalid.is_empty());
            let counts = predicted == v["predicted"].as_u64().unwrap() as usize
                && changed == v["changed"].as_u64().unwrap() as usize;
            let repeat = again == target && second == v["second"].as_u64().unwrap() as usize;
            let qualified = actual == expected && counts && repeat;
            if !qualified {
                failures.push(id.to_owned());
            }
            println!(
                "UNIFY_NORMALS_SDK {}",
                json!({
                    "id":id,"qualified":qualified,"geometry":actual.faces()==expected.faces(),
                    "channels":actual.texture_coordinates()==expected.texture_coordinates() && actual.vertex_colors()==expected.vertex_colors(),
                    "normals":actual.vertex_normals()==expected.vertex_normals(),"ngons":actual.ngons()==expected.ngons(),
                    "count_only":true,"source_unchanged":true,"counts":counts,"repeat":repeat,
                    "idempotent":again==actual && second==0,"predicted":predicted,"changed":changed,"second":second,
                    "flipped_faces":first_flips,"repeat_flipped_faces":repeat_flips,
                    "vertices":actual.vertices().len(),"faces":actual.face_count(),"maximum_error":0.
                })
            );
        }
    }
    assert_eq!(
        failures,
        [
            "unify_component_nonmanifold_01",
            "unify_component_nonmanifold_11",
            "unify_component_nonmanifold_21",
            "unify_component_tetra_201",
            "unify_component_tetra_211",
            "unify_component_tetra_221",
            "unify_component_mobius_011",
            "unify_component_mobius_111",
            "unify_component_mobius_211",
            "unify_runtime_tetra_201",
            "unify_runtime_nonmanifold_01",
        ],
        "SDK winding differences changed"
    );
}
