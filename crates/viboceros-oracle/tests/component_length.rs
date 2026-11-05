use serde_json::Value;
#[path = "support/observation.rs"]
mod observation;
#[path = "support/component_length.rs"]
mod support;
#[test]
fn independent_mesh_subd_and_surface_edges_match_native_lengths_and_selection() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let mut count = 0;
    for family in [
        "component_length_discovery",
        "component_length_followups",
        "component_length_workflows",
    ] {
        let request: Value = serde_json::from_slice(
            &std::fs::read(base.join(format!("fixtures/{family}.json"))).unwrap(),
        )
        .unwrap();
        let native = observation::read(&base, family);
        let actual = serde_json::to_value(
            viboceros_oracle::run_request(&serde_json::from_value(request).unwrap()).unwrap(),
        )
        .unwrap();
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
            assert_eq!(a["id"], b["id"]);
            count += 1;
            let (a, b) = (&a["value"], &b["value"]);
            for key in [
                "before",
                "after",
                "redo",
                "undo",
                "success",
                "source_unchanged",
            ] {
                assert_eq!(a[key], b[key], "{family} {key}");
            }
            if !b["trace"].is_null() {
                assert_eq!(a["trace"], b["trace"], "{family} trace");
            }
            if let Some((expected, epsilon)) = support::measurement(b) {
                assert!(
                    (a["length_mm"].as_f64().unwrap() - expected).abs() <= epsilon,
                    "{family} length"
                );
            } else {
                assert!(a["length_mm"].is_null());
            }
        }
    }
    assert_eq!(count, 42);
}
