//! Native public RebuildNormals inputs, including raw collapsed face records.
use serde_json::json;
#[path = "support/mesh_conversion.rs"]
mod raw_mesh;

#[allow(dead_code)]
#[path = "support/rebuild_normals.rs"]
mod fixtures;

#[test]
fn public_normal_rebuilding_matches_native_compaction_and_stored_normals() {
    for (_, rows) in fixtures::groups(false) {
        for row in rows {
            eprintln!("Rebuild normals {}", row["id"]);
            let v = &row["value"];
            let (source, invalid) = raw_mesh::mesh(&v["before"], false);
            assert!(invalid.is_empty());
            let (expected, invalid) = raw_mesh::mesh(&v["after"], false);
            assert!(invalid.is_empty());
            let actual = source.rebuilt_normals().unwrap();
            assert_eq!(actual, expected, "raw normal rebuild {}", row["id"]);
            let (compacted, _) = source.culled_unused_vertices();
            assert_eq!(actual.vertices(), compacted.vertices());
            assert_eq!(actual.faces(), compacted.faces());
            assert_eq!(actual.ngons(), compacted.ngons());
            assert_eq!(
                actual.texture_coordinates(),
                compacted.texture_coordinates()
            );
            assert_eq!(actual.vertex_colors(), compacted.vertex_colors());
            assert_eq!(actual.rebuilt_normals().unwrap(), actual);
            let mut maximum_face_error = 0f64;
            let mut collapsed = 0;
            for (face, normal) in v["after"]["face_normals"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let native: [f64; 3] = serde_json::from_value(normal.clone()).unwrap();
                match actual.polygon_face_normal(face) {
                    Ok(n) => {
                        for (a, b) in n.as_vector().to_array().into_iter().zip(native) {
                            maximum_face_error = maximum_face_error.max((a - b).abs());
                        }
                    }
                    Err(_) => {
                        assert_eq!(native, [0.; 3]);
                        collapsed += 1;
                    }
                }
            }
            assert!(maximum_face_error <= 1e-6);
            println!(
                "REBUILD_NORMALS_SDK {}",
                json!({"id":row["id"],"qualified":true,"raw_geometry":true,"channels":true,"normals":true,"ngons":true,"idempotent":true,"vertices":actual.vertices().len(),"faces":actual.face_count(),"collapsed_face_queries":collapsed,"maximum_face_error":maximum_face_error,"maximum_error":0.})
            );
        }
    }
}
