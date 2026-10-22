//! Stored normals follow explicit native angle-unwelding policies.
use serde_json::{Value, json};
#[path = "support/mesh_channels.rs"]
mod native;

#[test]
fn native_angle_unwelding_preserves_or_updates_stored_normals() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_unweld_channels.json"
    ))
    .unwrap();
    let mut failures = Vec::new();
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        assert_eq!(v["before"]["valid"], true);
        let source = native::mesh(&v["before"]);
        let original = source.clone();
        let (result, _) = source
            .unwelded_vertices_with_normal_update(
                v["input"]["angle"].as_f64().unwrap(),
                v["input"]["modify_normals"].as_bool().unwrap(),
            )
            .unwrap();
        assert_eq!(source, original);
        let differences = native::differences(&result, &v["after"]);
        println!(
            "MESH_UNWELD_CHANNEL_NATIVE {}",
            json!({"id":row["id"],"differences":differences,"qualified":differences.is_empty(),"vertices":result.vertices().len(),"faces":result.face_count()})
        );
        if !differences.is_empty() {
            failures.push((row["id"].clone(), differences));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
