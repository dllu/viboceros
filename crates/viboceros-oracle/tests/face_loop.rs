use serde_json::Value;
use std::io::Read;
#[test]
fn independent_face_loop_recipes_match_all_native_component_states() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "face_loop_discovery",
        "face_loop_commands",
        "component_pattern_preselection",
        "component_pattern_holdouts",
    ] {
        let request = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let actual =
            serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
        let raw = std::fs::read(base.join(format!("observations/{family}.json.gz"))).unwrap();
        let mut data = Vec::new();
        flate2::read::GzDecoder::new(raw.as_slice())
            .read_to_end(&mut data)
            .unwrap();
        let expected: Value = serde_json::from_slice(&data).unwrap();
        assert_eq!(
            actual["results"].as_array().unwrap().len(),
            expected["results"].as_array().unwrap().len()
        );
        for (a, b) in actual["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(expected["results"].as_array().unwrap())
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
    assert_eq!(count, 131);
}
