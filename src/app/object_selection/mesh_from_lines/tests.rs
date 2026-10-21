use super::*;
use crate::app::tests::test_app;
#[path = "../../../../crates/viboceros-command/tests/support/mesh_from_lines.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn line_network_getters_match_metadata_geometry_and_history() {
    for (prefix, rows) in native::groups() {
        for row in rows {
            let value = &row["value"];
            let scene = native::setup(value);
            let mut app = test_app();
            app.document = scene.document;
            enter(&mut app, &native::command(value));
            assert!(
                app.object_prompt
                    .as_ref()
                    .unwrap()
                    .mesh_from_lines
                    .is_some()
            );
            enter(&mut app, "");
            assert!(
                app.object_prompt.is_none(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            let mut error = native::check(&app.document, &scene.parents, value, "after_script");
            if native::has_output(value) {
                assert_eq!(app.document.undo_label(), Some("MeshFromLines"));
                app.document.undo().unwrap();
                error = error.max(native::check(&app.document, &scene.parents, value, "undo"));
                app.document.redo().unwrap();
                error = error.max(native::check(&app.document, &scene.parents, value, "redo"));
            } else {
                assert!(app.document.undo_label().is_none());
            }
            let supported = native::native_options_supported(value);
            println!(
                "{prefix}_GETTER {}",
                serde_json::json!({"id":row["id"],"qualified":error<1e-6&&supported,"geometry_qualified":error<1e-6,"native_options_supported":supported,"maximum_bidirectional_sample_error":error,"native_empty":!native::has_output(value),"sources_and_attributes_preserved":true,"grouped_history":true})
            );
            assert!(error < 1e-6, "{} {error:e}", row["id"]);
        }
    }
}
#[test]
fn numeric_options_cancel_selection_and_redo_are_independent() {
    let (_, rows) = native::groups().remove(0);
    let scene = native::setup(&rows[0]["value"]);
    let mut app = test_app();
    app.document = scene.document;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    app.document.clear_selection();
    enter(&mut app, "MeshFromLines");
    app.select_prompt_objects(scene.sources, SelectionMode::Replace);
    enter(&mut app, "MaxSidesPerFace");
    enter(&mut app, "2");
    assert!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .mesh_from_lines
            .as_ref()
            .unwrap()
            .value
            .is_some()
    );
    enter(&mut app, "6");
    enter(&mut app, "DeleteInput");
    enter(&mut app, "OutputLayer");
    enter(&mut app, "MeshFromLines source");
    assert!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .mesh_from_lines
            .as_ref()
            .unwrap()
            .options
            .output_layer
            .is_some()
    );
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().count(), scene.parents.len());
    let preferences = app
        .commands
        .mesh_from_lines_options(&[])
        .unwrap()
        .preferences;
    assert_eq!(preferences.max_sides, 6);
    assert!(preferences.delete_input);
    assert_eq!(
        viboceros_command::CommandRegistry::with_builtins()
            .mesh_from_lines_options(&[])
            .unwrap()
            .preferences,
        viboceros_command::mesh_from_lines::Preferences::default()
    );
}
#[test]
fn staged_network_rejects_an_unrelated_history_change() {
    let (_, rows) = native::groups().remove(0);
    let scene = native::setup(&rows[0]["value"]);
    let mut app = test_app();
    app.document = scene.document;
    enter(&mut app, "MeshFromLines");
    app.commands
        .execute(&mut app.document, "Point 8,8,8")
        .unwrap();
    let count = app.document.objects().count();
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().count(), count);
    assert_eq!(app.document.undo_label(), Some("Point"));
}
