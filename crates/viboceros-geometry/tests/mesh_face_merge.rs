#[path = "support/mesh_face_merge.rs"]
mod support;
#[test]
fn merge_face_components_match_all_48_native_mesh_storage_and_channels() {
    support::audit_all_native_mesh_storage_and_channels();
}
