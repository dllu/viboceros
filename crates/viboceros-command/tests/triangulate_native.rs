//! Independent public SDK observations, with raw channels and n-gon face lists.
use serde_json::{Value, json};
use viboceros_geometry::Tolerance;
#[path = "support/mesh_conversion.rs"]
mod raw_mesh;

#[test]
fn native_quad_triangulation_matches_face_slots_channels_and_ngons() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/triangulate_sdk.json"
    ))
    .unwrap();
    for row in data["results"].as_array().unwrap() {
        eprintln!("Triangulation {}", row["id"]);
        let value = &row["value"];
        assert_eq!(value["success"], true);
        assert_eq!(value["before"]["valid"], true);
        assert_eq!(value["after"]["valid"], true);
        let (source, invalid) = raw_mesh::mesh(&value["before"], false);
        assert!(invalid.is_empty());
        let (expected, invalid) = raw_mesh::mesh(&value["after"], false);
        assert!(invalid.is_empty());
        for tolerance in [
            Tolerance::DEFAULT,
            Tolerance::try_new(0.1, 1e-12, 0.5).unwrap(),
        ] {
            let (actual, converted) = source.triangulate_quads(tolerance).unwrap();
            assert_eq!(actual, expected, "raw triangulation {}", row["id"]);
            assert_eq!(value["converted"], converted);
            assert_eq!(actual.vertices(), source.vertices());
            assert_eq!(actual.vertex_colors(), source.vertex_colors());
            assert_eq!(actual.vertex_normals(), source.vertex_normals());
            assert_eq!(actual.texture_coordinates(), source.texture_coordinates());
        }
        println!(
            "TRIANGULATE_SDK {}",
            json!({"id":row["id"],"qualified":true,"raw_faces":true,"channels":true,"ngons":true,"tolerance_independent":true,"converted":value["converted"],"vertices":expected.vertices().len(),"faces":expected.face_count(),"maximum_error":0.})
        );
    }
}
