use serde_json::{Value, json};
#[path = "support/mesh_conversion.rs"]
mod raw_mesh;

#[test]
fn sdk_unification_matches_native_counts_winding_channels_and_readonly_queries() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("tools/rhino_oracle/observations").is_dir())
        .unwrap();
    let mut failures = Vec::new();
    for family in [
        "unify_normals_sdk",
        "unify_normals_extra",
        "unify_normals_holdouts",
    ] {
        let data: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("tools/rhino_oracle/observations")
                    .join(format!("{family}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            let id = row["id"].as_str().unwrap();
            eprintln!("Unify SDK {id}");
            assert_eq!(v["before"], v["after_count"]);
            let (source, invalid) = raw_mesh::mesh(&v["before"], false);
            assert!(invalid.is_empty());
            let (expected, invalid) = raw_mesh::mesh(&v["after"], false);
            assert!(invalid.is_empty());
            let before = source.clone();
            let predicted = source.normal_unification_count();
            let outcome = source.unified_normals();
            let Ok((actual, changed)) = outcome else {
                eprintln!("DECLINED {id}: {outcome:?}");
                failures.push(id.to_owned());
                continue;
            };
            assert_eq!(predicted, v["predicted"].as_u64().unwrap() as usize);
            assert_eq!(changed, v["changed"].as_u64().unwrap() as usize);
            assert_eq!(actual.vertices(), expected.vertices());
            let qualified = actual == expected;
            if !qualified {
                failures.push(id.to_owned());
            }
            assert_eq!(source, before);
            let (again, second) = actual.unified_normals().unwrap();
            assert_eq!(v["second"], second);
            let (target, invalid) = raw_mesh::mesh(&v["after_second"], false);
            assert!(invalid.is_empty());
            assert_eq!(again, actual);
            if qualified {
                assert_eq!(again, target);
            }
            println!(
                "UNIFY_NORMALS_SDK {}",
                json!({"id":id,"qualified":qualified,"geometry":actual.faces()==expected.faces() && actual.vertices()==expected.vertices(),"channels":actual.texture_coordinates()==expected.texture_coordinates() && actual.vertex_colors()==expected.vertex_colors(),"normals":actual.vertex_normals()==expected.vertex_normals(),"ngons":actual.ngons()==expected.ngons(),"count_only":true,"source_unchanged":true,"idempotent":true,"predicted":predicted,"changed":changed,"vertices":actual.vertices().len(),"faces":actual.face_count(),"maximum_error":0.})
            );
        }
    }
    assert_eq!(
        failures,
        ["unify_holdout_reindex_1032"],
        "Unmatched SDK cases changed"
    );
}
