use serde_json::Value;
#[path = "support/chain_selection.rs"]
mod chain_native;
#[path = "support/length_chain.rs"]
mod support;
#[test]
fn nested_chain_phases_lengths_selection_and_history_match_native_length() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "length_chain_discovery",
        "length_chain_controls",
        "length_chain_followups",
    ] {
        let request = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let actual =
            serde_json::to_value(viboceros_oracle::run_request(&request).unwrap()).unwrap();
        let native = support::observation(&base, family);
        assert_eq!(
            actual["results"].as_array().unwrap().len(),
            native["results"].as_array().unwrap().len()
        );
        for (a, b) in actual["results"]
            .as_array()
            .unwrap()
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            count += 1;
            assert_eq!(a["id"], b["id"]);
            let (a, b) = (&a["value"], &b["value"]);
            for key in ["before", "after", "redo", "undo"] {
                assert_eq!(a[key], support::normalize(&b[key]), "{family} {key}");
            }
            let trace = b["trace"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| support::normalize_trace(s, &b["after"]))
                .collect::<Vec<_>>();
            assert_eq!(a["trace"], serde_json::json!(trace), "{family} trace");
            for key in ["success", "source_unchanged"] {
                assert_eq!(a[key], b[key], "{key}");
            }
            if let Some((value, epsilon)) = support::length_mm(b["history"].as_str().unwrap()) {
                assert!((a["length_mm"].as_f64().unwrap() - value).abs() <= epsilon);
            } else {
                assert_eq!(a["length_mm"], Value::Null);
            }
        }
    }
    assert_eq!(count, 17);
}
