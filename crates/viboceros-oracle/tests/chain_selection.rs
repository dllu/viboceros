use serde_json::Value;
use std::io::Read;
#[path = "support/chain_selection.rs"]
mod support;
use support::normalize;
#[test]
fn independent_chain_sources_reproduce_native_selection_and_history() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "chain_selection_discovery",
        "chain_selection_reversed",
        "chain_selection_lines",
        "chain_selection_corner",
        "chain_selection_cycle",
        "chain_selection_surface",
        "chain_selection_mixed",
        "chain_selection_tolerance",
        "chain_selection_lifecycle",
        "chain_selection_curvature",
        "chain_selection_curvature_limits",
        "chain_selection_curvature_boundary",
        "chain_selection_acceptance",
        "chain_selection_branch",
        "chain_selection_prefer_curve",
        "chain_selection_forks",
        "chain_selection_angle_precision",
        "chain_selection_option_minimums",
        "chain_selection_option_boundary",
    ] {
        let request = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let actual =
            serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
        let path = base.join(format!("observations/{family}.json"));
        let bytes = if path.exists() {
            std::fs::read(path).unwrap()
        } else {
            let bytes = std::fs::read(path.with_extension("json.gz")).unwrap();
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(bytes.as_slice())
                .read_to_end(&mut raw)
                .unwrap();
            raw
        };
        let native: Value = serde_json::from_slice(&bytes).unwrap();
        for (a, b) in actual["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(a["id"], b["id"]);
            for key in ["before", "after", "redo", "undo"] {
                assert_eq!(
                    a["value"][key],
                    normalize(&b["value"][key]),
                    "{} {key}",
                    a["id"]
                );
            }
            let trace = b["value"]["trace"]
                .as_array()
                .unwrap()
                .iter()
                .map(normalize)
                .collect::<Vec<_>>();
            assert_eq!(
                a["value"]["trace"],
                serde_json::json!(trace),
                "{} trace",
                a["id"]
            );
            for key in ["success", "source_unchanged"] {
                assert_eq!(a["value"][key], b["value"][key], "{} {key}", a["id"]);
            }
        }
    }
    assert_eq!(count, 144);
}
