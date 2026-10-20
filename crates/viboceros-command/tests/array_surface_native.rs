#[allow(dead_code)]
#[path = "support/array_curve_surface.rs"]
mod array_curve_surface_test_support;
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[path = "support/array_surface.rs"]
mod support;
#[test]
fn native_surface_arrays_preview_outcomes_clouds_groups_selection_and_history() {
    let commands = viboceros_command::CommandRegistry::with_builtins();
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (mut doc, scene) = support::setup(&value);
            let id = op["id"].as_str().unwrap();
            scene.check(&doc, &value, "before", id);
            let result = support::execute(&scene, &mut doc, &commands, &op);
            result.unwrap();
            let mut max = scene.check(&doc, &value, "after_script", id);
            if value["after_script"]["objects"].as_array().unwrap().len()
                > value["before"]["objects"].as_array().unwrap().len()
            {
                assert_eq!(doc.undo_label(), Some("ArraySrf"));
                doc.undo().unwrap();
                max = max.max(scene.check(&doc, &value, "undo", id));
                assert!(doc.undo().unwrap().is_none());
                doc.redo().unwrap();
                max = max.max(scene.check(&doc, &value, "redo", id));
            } else {
                assert!(doc.undo_label().is_none());
            }
            println!(
                "ARRAY_SURFACE_API_AUDIT {}",
                serde_json::json!({"id":id,"maximum_error":max})
            );
        }));
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn collapsed_isocurve_array_with_command_picks_preserves_redo() {
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "surface_9")
        .unwrap();
    let (mut doc, scene) = support::setup(&value);
    let commands = viboceros_command::CommandRegistry::with_builtins();
    commands.execute(&mut doc, "Move 0,0,0 1,0,0").unwrap();
    doc.undo().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    let mut op = op;
    op["postselect"] = serde_json::json!(true);
    support::execute(&scene, &mut doc, &commands, &op).unwrap();
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(doc.undo_label().is_none());
    assert_eq!(doc.redo_label(), Some("Move"));
}
