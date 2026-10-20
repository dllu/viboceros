#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[path = "support/array_curve_surface.rs"]
mod support;
#[test]
fn native_surface_curve_array_clouds_groups_selection_and_real_history() {
    let commands = viboceros_command::CommandRegistry::with_builtins();
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (mut doc, scene) = support::setup(&value);
            let id = op["id"].as_str().unwrap();
            scene.check(&doc, &value, "before", id);
            scene.execute(&mut doc, &commands, &op, &value);
            let mut max = scene.check(&doc, &value, "after_script", id);
            if doc.objects().count() > value["before"]["objects"].as_array().unwrap().len() {
                assert_eq!(doc.undo_label(), Some("ArrayCrvOnSrf"));
                doc.undo().unwrap();
                max = max.max(scene.check(&doc, &value, "undo", id));
                assert!(doc.undo().unwrap().is_none());
                doc.redo().unwrap();
                max = max.max(scene.check(&doc, &value, "redo", id));
            } else {
                assert!(doc.undo_label().is_none());
                assert!(doc.redo_label().is_none());
            }
            println!(
                "ARRAY_CURVE_SURFACE_API_AUDIT {}",
                serde_json::json!({"id":id,"maximum_error":max})
            );
        }));
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
