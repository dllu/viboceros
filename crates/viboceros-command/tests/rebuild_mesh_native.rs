//! Independent static SDK rebuilds, with source immutability and all flags.
use serde_json::{Value, json};
#[path = "support/mesh_conversion.rs"]
mod raw_mesh;

#[test]
fn public_mesh_rebuilding_matches_independent_native_channel_and_geometry_policy() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/rebuild_mesh_sdk.json"
    ))
    .unwrap();
    for row in data["results"].as_array().unwrap() {
        eprintln!("Rebuild mesh SDK {}", row["id"]);
        let v = &row["value"];
        assert_eq!(v["success"], true);
        assert_eq!(v["source_after"], v["before"]);
        let (source, invalid) = raw_mesh::mesh(&v["before"], false);
        assert!(invalid.is_empty());
        let (expected, invalid) = raw_mesh::mesh(&v["after"], false);
        assert!(invalid.is_empty());
        let flags = v["input"]["options"]
            .as_array()
            .map_or([false, false], |a| {
                [a[0].as_bool().unwrap(), a[1].as_bool().unwrap()]
            });
        let before = source.clone();
        let actual = source.rebuilt(flags[0], flags[1]).unwrap();
        assert_eq!(actual, expected, "raw rebuilt mesh {}", row["id"]);
        assert_eq!(source, before);
        assert!(actual.ngons().is_empty());
        assert_eq!(actual.rebuilt(flags[0], flags[1]).unwrap(), actual);
        println!(
            "REBUILD_MESH_SDK {}",
            json!({"id":row["id"],"qualified":true,"geometry":true,"channels":true,"normals":true,"ngons":true,"source_unchanged":true,"idempotent":true,"vertices":actual.vertices().len(),"faces":actual.face_count(),"preserve_uv":flags[0],"preserve_colors":flags[1],"maximum_error":0.})
        );
    }
}
