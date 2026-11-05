//! Compare tagged oriented face sets, independent of native component numbering.
use super::subd_profile_comparison as geometry;
use serde_json::Value;
pub fn compare(actual: &Value, native: &Value, path: &str, epsilon: f64) {
    let mut actual = geometry::canonical_edges(&geometry::align_vertices(actual, native));
    let mut native = geometry::canonical_edges(native);
    sort_faces(&mut actual);
    sort_faces(&mut native);
    assert!(
        std::panic::catch_unwind(|| geometry::compare(&actual, &native, path, epsilon)).is_ok(),
        "{path}: native SubD comparison failed"
    );
}
fn sort_faces(value: &mut Value) {
    match value {
        Value::Array(a) => a.iter_mut().for_each(sort_faces),
        Value::Object(a) => {
            if a.contains_key("vertex_tags") {
                // Face starts were cyclically normalized after control-vertex
                // correspondence. Preserve winding, multiplicity and incidence.
                a.get_mut("faces")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .sort_by_key(|f| {
                        f.as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap())
                            .collect::<Vec<_>>()
                    });
            }
            a.values_mut().for_each(sort_faces);
        }
        _ => {}
    }
}
