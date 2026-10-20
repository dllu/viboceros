use viboceros_command::CommandRegistry;
#[allow(dead_code)]
#[path = "support/grip_transform.rs"]
mod grip_fixtures;
#[allow(dead_code)]
#[path = "support/splop_grips.rs"]
mod grips;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[path = "support/splop_finish.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/splop.rs"]
mod whole;
#[test]
fn native_splop_enter_preserves_preselection_and_clears_command_first_sources() {
    let mut failures = Vec::new();
    for (op, recipe, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let gap = if op["kind"] == "Whole" {
                let (mut doc, sources) = whole::setup(&recipe, &value);
                sources.execute(&mut doc, &recipe, &CommandRegistry::with_builtins());
                let mut gap = sources.check(&doc, &recipe, &value, "after_script");
                doc.undo().unwrap();
                gap = gap.max(sources.check(&doc, &recipe, &value, "undo"));
                doc.redo().unwrap();
                gap.max(sources.check(&doc, &recipe, &value, "redo"))
            } else {
                let (mut doc, sources) = grips::setup(&recipe, &value);
                sources.execute(&mut doc, &recipe, &CommandRegistry::with_builtins());
                let id = op["id"].as_str().unwrap();
                let mut gap = sources.check(&doc, &value, "after_script", id);
                doc.undo().unwrap();
                gap = gap.max(sources.check(&doc, &value, "undo", id));
                doc.redo().unwrap();
                gap.max(sources.check(&doc, &value, "redo", id))
            };
            println!(
                "SPLOP_FINISH_API_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
