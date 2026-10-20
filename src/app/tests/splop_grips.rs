use super::*;
use crate::splop_grips_test_support as support;
#[test]
fn splop_grip_prompts_replay_native_nets_display_selection_groups_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let mut app = test_app();
            let (doc, sources) = support::setup(&op, &value);
            app.document = doc;
            let id = op["id"].as_str().unwrap();
            sources.check(&app.document, &value, "before", id);
            assert!(app.try_start_interactive_command(&format!(
                "Splop {}",
                support::Sources::options(&op).arguments()
            )));
            assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
            assert!(app.try_continue_orient_surface("2"));
            assert!(app.accept_orient_surface(sources.guide, None));
            for (radius, angle) in support::placements(&op) {
                assert!(app.accept_drafting_point(sources.target));
                assert!(app.accept_drafting_point(sources.direction(radius, angle)));
            }
            if app.active_command.is_some() {
                if op["finish"] == "Cancel" {
                    app.cancel_interactive_command(true)
                } else {
                    assert!(app.try_continue_orient_surface("Enter"));
                }
            }
            // The retained macro sends a second Cancel after Splop has ended.
            if op["copy"] == true && op["finish"] != "Cancel" {
                app.cancel_current_prompt_or_selection();
            }
            assert!(app.active_command.is_none());
            assert!(app.orient_surface_session.is_none());
            let mut gap = sources.check(&app.document, &value, "after_script", id);
            assert_eq!(app.document.undo_label(), Some("Splop"));
            assert!(app.document.undo().unwrap().is_some());
            gap = gap.max(sources.check(&app.document, &value, "undo", id));
            assert!(app.document.redo().unwrap().is_some());
            gap = gap.max(sources.check(&app.document, &value, "redo", id));
            println!(
                "SPLOP_GRIP_UI_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
