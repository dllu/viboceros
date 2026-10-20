use super::*;
use crate::{
    splop_finish_test_support as support, splop_grips_test_support as grips,
    splop_test_support as whole,
};
#[test]
fn splop_enter_and_escape_preserve_native_preselected_objects_and_grips() {
    let mut failures = Vec::new();
    for (op, recipe, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let mut app = test_app();
            let id = op["id"].as_str().unwrap();
            let gap = if op["kind"] == "Whole" {
                let (doc, sources) = whole::setup(&recipe, &value);
                app.document = doc;
                assert!(app.try_start_interactive_command("Splop Copy=Yes"));
                if recipe["postselect"] == true {
                    assert!(app.object_prompt.is_some());
                    app.document
                        .select_objects_direct(sources.ids.iter().copied(), SelectionMode::Replace)
                        .unwrap();
                    assert!(app.try_continue_transform_source_prompt("Enter"));
                }
                assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
                assert!(app.try_continue_orient_surface("2"));
                assert!(app.accept_orient_surface(sources.guide, None));
                assert!(app.accept_drafting_point(sources.target));
                assert!(app.accept_drafting_point(sources.direction(4., 0.)));
                if op["finish"] == "Enter" {
                    assert!(app.try_continue_orient_surface("Enter"));
                } else {
                    app.cancel_interactive_command(true);
                }
                let mut gap = sources.check(&app.document, &recipe, &value, "after_script");
                app.document.undo().unwrap();
                gap = gap.max(sources.check(&app.document, &recipe, &value, "undo"));
                app.document.redo().unwrap();
                gap.max(sources.check(&app.document, &recipe, &value, "redo"))
            } else {
                let (doc, sources) = grips::setup(&recipe, &value);
                app.document = doc;
                assert!(app.try_start_interactive_command("Splop Copy=Yes"));
                assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
                assert!(app.try_continue_orient_surface("2"));
                assert!(app.accept_orient_surface(sources.guide, None));
                assert!(app.accept_drafting_point(sources.target));
                assert!(app.accept_drafting_point(sources.direction(4., 0.)));
                if op["finish"] == "Enter" {
                    assert!(app.try_continue_orient_surface("Enter"));
                } else {
                    app.cancel_interactive_command(true);
                }
                let mut gap = sources.check(&app.document, &value, "after_script", id);
                app.document.undo().unwrap();
                gap = gap.max(sources.check(&app.document, &value, "undo", id));
                app.document.redo().unwrap();
                gap.max(sources.check(&app.document, &value, "redo", id))
            };
            assert!(app.active_command.is_none());
            assert!(app.orient_surface_session.is_none());
            println!(
                "SPLOP_FINISH_UI_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
