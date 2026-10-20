#[allow(dead_code)]
#[path = "support/array_curve_surface.rs"]
mod array_curve_surface_test_support;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[path = "support/curve_array.rs"]
mod support;
#[test]
fn native_curve_arrays_placements_groups_selection_and_history() {
    let commands = viboceros_command::CommandRegistry::with_builtins();
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (mut doc, scene) = support::setup(&value);
            let id = op["id"].as_str().unwrap();
            support::check(&scene, &doc, &op, &value, "before");
            support::execute(&mut doc, &scene, &commands, &op, &value);
            let mut max = support::check(&scene, &doc, &op, &value, "after_script");
            if value["after_script"]["objects"].as_array().unwrap().len()
                > value["before"]["objects"].as_array().unwrap().len()
            {
                assert_eq!(doc.undo_label(), Some("ArrayCrv"));
                doc.undo().unwrap();
                max = max.max(support::check(&scene, &doc, &op, &value, "undo"));
                assert!(doc.undo().unwrap().is_none());
                doc.redo().unwrap();
                max = max.max(support::check(&scene, &doc, &op, &value, "redo"));
            } else {
                assert!(doc.undo_label().is_none());
            }
            println!(
                "CURVE_ARRAY_API_AUDIT {}",
                serde_json::json!({"id":id,"maximum_error":max,"qualification":support::qualification(&op)})
            );
        }));
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
