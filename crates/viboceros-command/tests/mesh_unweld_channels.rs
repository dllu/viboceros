//! ModifyNormals affects stored data while preserving document history.
use serde_json::Value;
#[path = "support/mesh_seams.rs"]
mod native;

#[test]
fn unweld_normal_policies_preserve_metadata_selection_and_undo_redo() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/mesh_unweld_channels.json"
    ))
    .unwrap();
    assert_eq!(data["results"].as_array().unwrap().len(), 18);
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        native::check(
            row["id"].as_str().unwrap(),
            v,
            &format!(
                "Unweld Angle={} ModifyNormals={}",
                v["input"]["angle"].as_f64().unwrap().to_degrees(),
                if v["input"]["modify_normals"].as_bool().unwrap() {
                    "Yes"
                } else {
                    "No"
                }
            ),
            "Unweld",
        );
        if v["input"]["modify_normals"] == true {
            native::check(
                row["id"].as_str().unwrap(),
                v,
                &format!(
                    "Unweld Angle={}",
                    v["input"]["angle"].as_f64().unwrap().to_degrees()
                ),
                "Unweld",
            );
        }
    }
}
