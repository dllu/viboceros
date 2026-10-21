use super::*;
use crate::app::tests::test_app;
#[path = "../../../../crates/viboceros-command/tests/support/mesh_patch.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn native_mesh_patch_getters_preserve_sources_attributes_and_grouped_history() {
    getter_replay(native::rows(), "MESH_PATCH_GETTER", false);
}
#[test]
fn native_mesh_patch_crossing_getters_keep_compatibility_gaps_visible() {
    for (group, rows) in native::crossing_groups() {
        getter_replay(rows, &format!("MESH_PATCH_{group}_GETTER"), true);
    }
}
fn getter_replay(
    rows: Vec<(serde_json::Value, serde_json::Value)>,
    prefix: &str,
    diagnostic: bool,
) {
    let mut failures = Vec::new();
    for (op, v) in rows {
        let scene = native::setup(&v);
        let command = native::command(&scene, &v);
        let mut app = test_app();
        app.document = scene.document;
        enter(&mut app, command.split(" Sources=").next().unwrap());
        assert_eq!(
            app.object_prompt
                .as_ref()
                .unwrap()
                .mesh_patch
                .as_ref()
                .unwrap()
                .phase,
            Phase::Holes
        );
        if !scene.holes.is_empty() {
            app.select_prompt_objects(scene.holes, SelectionMode::Replace);
        }
        if let Some(id) = scene.surface {
            enter(&mut app, "StartingSurface");
            app.select_prompt_objects([id], SelectionMode::Replace);
            enter(&mut app, "");
            assert_eq!(
                app.object_prompt
                    .as_ref()
                    .unwrap()
                    .mesh_patch
                    .as_ref()
                    .unwrap()
                    .phase,
                Phase::Holes
            );
        }
        enter(&mut app, "");
        assert!(
            app.object_prompt.is_none(),
            "{} {:?}",
            op["id"],
            app.command_log
        );
        if v["input"]["expect_failure"] == true {
            assert!(app.document.undo_label().is_none());
            let error = native::check(&app.document, &scene.parents, &v, "after_script");
            println!(
                "{prefix} {}",
                serde_json::json!({"id":op["id"],"qualified":true,"native_failure":true,"maximum_bidirectional_sample_error":error})
            );
            continue;
        }
        let mut comparison = native::compare(&app.document, &scene.parents, &v, "after_script");
        app.document.undo().unwrap();
        comparison.merge(native::compare(&app.document, &scene.parents, &v, "undo"));
        app.document.redo().unwrap();
        comparison.merge(native::compare(&app.document, &scene.parents, &v, "redo"));
        println!(
            "{prefix} {}",
            serde_json::json!({"id":op["id"],"qualified":comparison.qualified(),"maximum_bidirectional_sample_error":comparison.error,"counts_match":comparison.counts_match,"source_metadata_preserved":true,"grouped_history":true})
        );
        if !comparison.qualified() {
            failures.push(op["id"].as_str().unwrap().to_owned());
        }
    }
    if diagnostic {
        native::assert_known_gaps(&failures);
    } else {
        assert!(failures.is_empty(), "{failures:?}");
    }
}
#[test]
fn staged_source_picking_cancel_options_and_redo_remain_independent() {
    let (_, v) = native::rows().remove(0);
    let scene = native::setup(&v);
    let mut app = test_app();
    app.document = scene.document;
    app.commands
        .execute(&mut app.document, "Point 8,8,8")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    app.document.clear_selection();
    enter(&mut app, "MeshPatch");
    assert_eq!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .mesh_patch
            .as_ref()
            .unwrap()
            .phase,
        Phase::Sources
    );
    app.select_prompt_objects(scene.sources.clone(), SelectionMode::Replace);
    enter(&mut app, "");
    enter(&mut app, "AngleTolerance");
    enter(&mut app, "bad-value");
    assert_eq!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .mesh_patch
            .as_ref()
            .unwrap()
            .options
            .preferences
            .angle,
        15.
    );
    enter(&mut app, "30");
    enter(&mut app, "TrimBack");
    assert!(!app.object_prompt.as_ref().unwrap().description.options[0].value);
    enter(&mut app, "MainSegmentCount");
    enter(&mut app, "3");
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().count(), scene.parents.len());
    let preferences = app.commands.mesh_patch_options(&[]).unwrap().preferences;
    assert_eq!(preferences.angle, 30.);
    assert!(!preferences.trim_back);
    assert_eq!(preferences.divisions, 3);
    assert_eq!(
        viboceros_command::CommandRegistry::with_builtins()
            .mesh_patch_options(&[])
            .unwrap()
            .preferences,
        solver::Preferences::default()
    );
}
#[test]
fn stale_history_rejects_the_staged_patch_without_an_extra_edit() {
    let (_, v) = native::rows().remove(0);
    let scene = native::setup(&v);
    let mut app = test_app();
    app.document = scene.document;
    enter(&mut app, "MeshPatch");
    app.commands
        .execute(&mut app.document, "Point 8,8,8")
        .unwrap();
    let count = app.document.objects().count();
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().count(), count);
    assert_eq!(app.document.undo_label(), Some("Point"));
}
