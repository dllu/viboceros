#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[allow(dead_code)]
#[path = "support/orient_curve_edge.rs"]
mod support;
#[test]
fn native_curve_to_edge_geometry_and_history() {
    let commands = viboceros_command::CommandRegistry::with_builtins();
    let mut failures = Vec::new();
    for (op, v) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (mut doc, scene) = support::setup(&op, &v);
            support::execute(&mut doc, &scene, &commands, &op);
            let mut error = support::check(&doc, &scene, &v, "after_script");
            if doc.undo_label().is_some() {
                doc.undo().unwrap();
                error = error.max(support::check(&doc, &scene, &v, "undo"));
                assert!(doc.undo().unwrap().is_none());
                doc.redo().unwrap();
                error = error.max(support::check(&doc, &scene, &v, "redo"));
            }
            println!(
                "ORIENT_EDGE_API {}",
                serde_json::json!({"id":op["id"],"maximum_error":error})
            );
            assert!(error < 1e-6, "{error}");
        }));
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
