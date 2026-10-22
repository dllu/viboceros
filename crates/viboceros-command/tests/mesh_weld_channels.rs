//! Stored-channel welding commits one identity-preserving document edit.
use serde_json::Value;
#[path = "support/mesh_seams.rs"]
mod native;

#[test]
fn weld_retains_metadata_groups_and_stored_channels_through_undo_redo() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cases = Vec::new();
    for suffix in ["", "_options"] {
        let data: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!(
                "tools/rhino_oracle/observations/mesh_weld_channels{suffix}.json"
            )))
            .unwrap(),
        )
        .unwrap();
        cases.extend(
            data["results"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| {
                    let name = r["id"].as_str().unwrap();
                    ["pair_flat", "pair_bent_above", "nonmanifold"].contains(&name)
                        || name.starts_with("transparency_")
                })
                .cloned(),
        );
    }
    assert_eq!(cases.len(), 15);
    for row in cases {
        native::check(
            row["id"].as_str().unwrap(),
            &row["value"],
            &format!(
                "Weld Angle={}",
                row["value"]["input"]["angle"]
                    .as_f64()
                    .unwrap()
                    .to_degrees()
            ),
            "Weld",
        );
    }
}
