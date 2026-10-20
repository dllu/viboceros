use super::*;
use crate::splop_test_support as support;
#[test]
fn splop_sphere_prompts_replay_native_geometry_selection_groups_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let mut app = test_app();
            let (doc, sources) = support::setup(&op, &value);
            app.document = doc;
            app.viewports[app.active_viewport].set_construction_plane(sources.plane);
            if op["postselect"] == true {
                app.document.clear_selection();
            }
            let input = format!("Splop {}", sources.options(&op).arguments());
            assert!(
                app.try_start_interactive_command(&input),
                "{} start",
                op["id"]
            );
            if op["postselect"] == true {
                assert!(app.object_prompt.is_some());
                app.document
                    .select_objects_direct(
                        sources.ids.iter().copied(),
                        viboceros_document::SelectionMode::Replace,
                    )
                    .unwrap();
                assert!(app.try_continue_transform_source_prompt("Enter"));
            }
            assert!(matches!(
                app.active_command,
                Some(InteractiveCommand::Splop { .. })
            ));
            assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
            assert!(app.accept_drafting_point(sources.reference));
            assert!(app.accept_orient_surface(sources.guide, None));
            for (radius, angle) in support::spheres(&op) {
                assert!(app.accept_drafting_point(sources.target));
                if let Some(locked) = op["locked_radius"].as_f64() {
                    assert!(app.try_continue_orient_surface(&locked.to_string()));
                }
                assert!(
                    app.accept_drafting_point(sources.direction(radius, angle)),
                    "{} sphere point",
                    op["id"]
                );
            }
            if app.active_command.is_some() {
                if op["finish"] == "Cancel" {
                    app.cancel_interactive_command(true);
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
            let fit = sources.check_fit(&app.document, &op);
            println!(
                "SPLOP_FIT_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":fit})
            );
            let mut gap = sources.check(&app.document, &op, &value, "after_script");
            assert_eq!(app.document.undo_label(), Some("Splop"));
            assert!(app.document.undo().unwrap().is_some());
            gap = gap.max(sources.check(&app.document, &op, &value, "undo"));
            assert!(app.document.redo().unwrap().is_some());
            gap = gap.max(sources.check(&app.document, &op, &value, "redo"));
            println!(
                "SPLOP_UI_AUDIT {}",
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
fn splop_numeric_radii_lock_size_and_keep_surface_orientation_preferences_separate() {
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "splop_0")
        .unwrap();
    let mut app = test_app();
    let (doc, sources) = support::setup(&op, &value);
    app.document = doc;
    let before_options = app.commands.orient_surface_options();
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
    assert!(app.try_continue_orient_surface("2mm"));
    assert!(app.picking_orient_surface());
    assert!(app.accept_orient_surface(sources.guide, None));
    assert!(app.accept_drafting_point(sources.target));
    assert!(app.try_continue_orient_surface("4mm"));
    // A radius lock still requires a direction; its point distance is ignored.
    assert!(app.active_command.is_some());
    assert!(app.document.undo_label().is_none());
    assert!(app.accept_drafting_point(sources.direction(9., 0.)));
    assert!(app.active_command.is_none());
    sources.check(&app.document, &op, &value, "after_script");
    assert_eq!(app.commands.orient_surface_options(), before_options);
    assert_eq!(app.commands.begin_copy_options("Splop"), Some(false));
}

#[test]
fn splop_unaccepted_input_and_stale_target_preserve_redo_and_original_geometry() {
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "splop_0")
        .unwrap();
    let mut app = test_app();
    let (doc, sources) = support::setup(&op, &value);
    app.document = doc;
    app.commands
        .execute(&mut app.document, "Move 0,0,0 1,0,0")
        .unwrap();
    app.document.undo().unwrap();
    let original = app
        .document
        .object(sources.ids[0])
        .unwrap()
        .geometry()
        .clone();
    assert!(app.try_start_interactive_command("Splop"));
    assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
    assert!(app.try_continue_orient_surface("-2"));
    assert!(app.document.undo_label().is_none());
    assert_eq!(app.document.redo_label(), Some("Move"));
    assert!(app.try_continue_orient_surface("2"));
    assert!(app.accept_orient_surface(sources.guide, None));
    assert!(app.accept_drafting_point(sources.target));
    assert!(app.try_continue_orient_surface("Copy"));
    assert!(app.try_continue_orient_surface("Maybe"));
    assert!(!app.accept_drafting_point(sources.direction(4., 0.)));
    assert!(app.try_continue_orient_surface("No"));
    assert!(app.try_continue_orient_surface("0"));
    app.cancel_interactive_command(true);
    assert_eq!(
        app.document.object(sources.ids[0]).unwrap().geometry(),
        &original
    );
    assert!(app.document.undo_label().is_none());
    assert_eq!(app.document.redo_label(), Some("Move"));

    assert!(app.try_start_interactive_command("Splop Copy=No"));
    assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
    assert!(app.try_continue_orient_surface("2"));
    assert!(app.accept_orient_surface(sources.guide, None));
    app.document
        .set_objects_visibility([sources.guide], false)
        .unwrap();
    assert!(!app.accept_drafting_point(sources.target));
    assert!(app.picking_orient_surface());
    assert_eq!(
        app.document.object(sources.ids[0]).unwrap().geometry(),
        &original
    );
}

#[test]
fn splop_empty_reference_radius_uses_the_native_sphere_default() {
    let (op, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "splop_0")
        .unwrap();
    let mut app = test_app();
    let (doc, sources) = support::setup(&op, &value);
    app.document = doc;
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    assert!(app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
    assert!(app.try_continue_orient_surface("Enter"));
    assert!(app.picking_orient_surface());
    assert!(app.accept_orient_surface(sources.guide, None));
    assert!(app.accept_drafting_point(sources.target));
    assert!(app.accept_drafting_point(sources.direction(2., 0.)));
    sources.check(&app.document, &op, &value, "after_script");
}
