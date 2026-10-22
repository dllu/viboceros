//! Independent native geometry, order and stored-channel comparisons.
use serde_json::{Value, json};
#[path = "support/mesh_channels.rs"]
mod native;
use native::mesh;

#[test]
fn native_whole_mesh_welding_tracks_channels_and_remaining_differences() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let known: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/mesh_weld_channels_gaps.json"
    ))
    .unwrap();
    let mut changes = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for suffix in [
        "",
        "_followup",
        "_prefixes",
        "_pairs",
        "_stress",
        "_trace",
        "_options",
    ] {
        let family = format!("mesh_weld_channels{suffix}");
        let data: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("tools/rhino_oracle/observations/{family}.json")))
                .unwrap(),
        )
        .unwrap();
        for row in data["results"].as_array().unwrap() {
            let v = &row["value"];
            if v["before"]["valid"] != true {
                println!(
                    "MESH_WELD_CHANNEL_DIAGNOSTIC {}",
                    json!({"id":row["id"],"reason":"native source is invalid"})
                );
                continue;
            }
            let source = mesh(&v["before"]);
            let original = source.clone();
            let actual = source
                .welded_vertices(v["input"]["angle"].as_f64().unwrap())
                .unwrap()
                .0;
            assert_eq!(source, original);
            let differences = native::differences(&actual, &v["after"]);
            println!(
                "MESH_WELD_CHANNEL_NATIVE {}",
                json!({"family":family,"id":row["id"],"differences":differences,"qualified":differences.is_empty(),"vertices":actual.vertices().len(),"faces":actual.face_count()})
            );
            let expected_gaps = known
                .get(row["id"].as_str().unwrap())
                .cloned()
                .unwrap_or(json!([]));
            if json!(differences) != expected_gaps {
                changes.push(format!("{}: {differences:?}", row["id"]));
            }
            if !differences.is_empty() {
                seen.insert(row["id"].as_str().unwrap().to_owned());
            }
        }
    }
    assert!(
        changes.is_empty(),
        "new regressions or resolved gaps; update measured limits:\n{}",
        changes.join("\n")
    );
    assert_eq!(seen, known.as_object().unwrap().keys().cloned().collect());
}
