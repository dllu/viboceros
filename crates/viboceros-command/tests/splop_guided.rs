#[allow(dead_code)]
#[path = "support/grip_transform.rs"]
mod grip_fixtures;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[allow(dead_code)]
#[path = "support/splop_guided.rs"]
mod support;
#[test]
fn native_splop_guided_reference_clouds_selection_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let (mut doc, sources) = support::setup(&value);
            let id = op["id"].as_str().unwrap();
            sources.check(&doc, &value, "before", id);
            sources.execute(&mut doc, &op, &value);
            let mut gap = sources.check(&doc, &value, "after_script", id);
            if op["finish"] == "CancelReference" {
                assert!(doc.undo_label().is_none());
                assert!(doc.redo_label().is_none());
            } else {
                assert_eq!(doc.undo_label(), Some("Splop"));
                assert!(doc.undo().unwrap().is_some());
                gap = gap.max(sources.check(&doc, &value, "undo", id));
                assert!(doc.redo().unwrap().is_some());
                gap = gap.max(sources.check(&doc, &value, "redo", id));
            }
            println!(
                "SPLOP_GUIDED_API_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
