use super::*;
use crate::{splop_reference_test_support as support, splop_test_support as whole};
#[test]
fn splop_reference_sphere_prompts_replay_native_clouds_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let op = support::mapped(&op);
            let (doc, source) = whole::setup(&op, &value);
            let mut app = test_app();
            app.document = doc;
            app.viewports[app.active_viewport].set_construction_plane(source.plane);
            assert!(app.try_start_interactive_command("Splop Copy=No"));
            for token in support::tokens(&value) {
                app.command_input = token.into();
                app.run_command();
                assert!(app.document.undo_label().is_none(), "{} {token}", op["id"]);
            }
            assert!(
                app.picking_orient_surface(),
                "{} {:?}",
                op["id"],
                app.command_log
            );
            assert!(app.accept_orient_surface(source.guide, None));
            assert!(app.accept_drafting_point(source.target));
            assert!(app.accept_drafting_point(source.direction(4., 0.)));
            assert!(app.active_command.is_none());
            let mut gap = source.check(&app.document, &op, &value, "after_script");
            assert_eq!(app.document.undo_label(), Some("Splop"));
            app.document.undo().unwrap();
            gap = gap.max(source.check(&app.document, &op, &value, "undo"));
            app.document.redo().unwrap();
            gap = gap.max(source.check(&app.document, &op, &value, "redo"));
            assert!(gap < 1e-9, "{} {gap}", op["id"]);
            println!(
                "SPLOP_REFERENCE_UI_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn splop_reference_invalid_points_keep_the_prompt_and_existing_redo() {
    let (op, value) = support::rows().remove(0);
    for prefix in [
        vec!["_3Point", "w-1,0,0", "w1,0,0"],
        vec!["_4Point", "w2,0,0", "w0,2,0", "w-2,0,0"],
    ] {
        let (doc, source) = whole::setup(&support::mapped(&op), &value);
        let mut app = test_app();
        app.document = doc;
        app.commands
            .execute(&mut app.document, "Move 0,0,0 1,0,0")
            .unwrap();
        app.document.undo().unwrap();
        let original = app
            .document
            .object(source.ids[0])
            .unwrap()
            .geometry()
            .clone();
        assert!(app.try_start_interactive_command("Splop Copy=No"));
        for token in prefix {
            app.command_input = token.into();
            app.run_command();
        }
        assert!(!app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
        assert_eq!(app.document.redo_label(), Some("Move"));
        assert!(app.document.undo_label().is_none());
        app.cancel_interactive_command(true);
        assert_eq!(
            app.document.object(source.ids[0]).unwrap().geometry(),
            &original
        );
        assert_eq!(app.document.redo_label(), Some("Move"));
    }
}

#[test]
fn splop_reference_size_modes_share_the_default_unit_radius() {
    let (op, value) = support::rows().remove(0);
    for mode in ["Radius", "Diameter", "Circumference", "Area"] {
        let op = support::mapped(&op);
        let (doc, source) = whole::setup(&op, &value);
        let mut app = test_app();
        app.document = doc;
        assert!(app.try_start_interactive_command("Splop Copy=No"));
        assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
        assert!(app.try_continue_orient_surface(mode));
        assert!(app.try_continue_orient_surface("Enter"));
        assert!(app.accept_orient_surface(source.guide, None));
        assert!(app.accept_drafting_point(source.target));
        assert!(app.accept_drafting_point(source.direction(2., 0.)));
        assert!(source.check(&app.document, &op, &value, "after_script") < 1e-9);
    }
}
