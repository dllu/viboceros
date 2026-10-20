use viboceros_command::{CommandContext, CommandRegistry};
#[allow(dead_code)]
#[path = "../src/morph_test_support.rs"]
mod morph_fixtures;
#[allow(dead_code)]
#[path = "support/splop.rs"]
mod splop_test_support;
#[allow(dead_code)]
#[path = "support/splop_reference.rs"]
mod support;
#[test]
fn native_splop_reference_spheres_replay_full_clouds_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let def = support::reference(&op, &value);
            let op = support::mapped(&op);
            let (mut doc, source) = splop_test_support::setup(&op, &value);
            let registry = CommandRegistry::with_builtins();
            registry
                .execute_in_context(
                    &mut doc,
                    &format!(
                        "Splop {} {} {} 4 Surface={} Copy=No",
                        splop_test_support::token(def.frame.origin()),
                        def.radius,
                        splop_test_support::token(source.target),
                        source.guide
                    ),
                    CommandContext {
                        construction_plane: def.frame,
                    },
                )
                .unwrap();
            let mut gap = source.check(&doc, &op, &value, "after_script");
            doc.undo().unwrap();
            gap = gap.max(source.check(&doc, &op, &value, "undo"));
            doc.redo().unwrap();
            gap = gap.max(source.check(&doc, &op, &value, "redo"));
            assert!(gap < 1e-9, "{} {gap}", op["id"]);
            println!(
                "SPLOP_REFERENCE_API_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
