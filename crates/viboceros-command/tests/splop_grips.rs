use viboceros_command::CommandRegistry;
#[allow(dead_code)]
#[path = "support/grip_transform.rs"]
mod grip_fixtures;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[allow(dead_code)]
#[path = "support/splop_grips.rs"]
mod support;
#[test]
fn native_splop_grips_match_owner_nets_display_selection_groups_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let (mut doc, sources) = support::setup(&op, &value);
            let id = op["id"].as_str().unwrap();
            sources.check(&doc, &value, "before", id);
            sources.execute(&mut doc, &op, &CommandRegistry::with_builtins());
            // Replay the captured idle Cancel after Enter separately from Splop completion.
            if op["copy"] == true && op["finish"] != "Cancel" {
                doc.clear_selection();
            }
            let mut maximum = sources.check(&doc, &value, "after_script", id);
            assert!(doc.undo().unwrap().is_some());
            maximum = maximum.max(sources.check(&doc, &value, "undo", id));
            assert!(doc.redo().unwrap().is_some());
            maximum = maximum.max(sources.check(&doc, &value, "redo", id));
            println!(
                "SPLOP_GRIP_API_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":maximum})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
