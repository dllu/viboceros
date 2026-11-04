use serde_json::Value;
use std::io::Read;
#[test]
fn independent_polygon_recipes_match_all_native_loop_and_ring_workflow_states() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "edge_loop_workflow_discovery",
        "edge_loop_workflows",
        "edge_ring_discovery",
        "edge_ring_commands",
        "edge_ring_ngons",
    ] {
        let query = std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap();
        let request = serde_json::from_slice(&query).unwrap();
        let result =
            serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
        let raw = std::fs::read(base.join(format!("observations/{family}.json.gz"))).unwrap();
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(raw.as_slice())
            .read_to_end(&mut data)
            .unwrap();
        let data: Value = serde_json::from_slice(&data).unwrap();
        for (a, b) in result["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(data["results"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(a["id"], b["id"]);
            for key in [
                "before",
                "trace",
                "after",
                "redo",
                "undo",
                "success",
                "source_unchanged",
            ] {
                assert_eq!(a["value"][key], b["value"][key], "{} {key}", a["id"]);
            }
        }
    }
    assert_eq!(count, 129);
}
