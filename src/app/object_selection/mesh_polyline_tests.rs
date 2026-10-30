use super::*;
use crate::app::{closed_polyline_native_tests as native, tests::test_app};

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn mesh_polyline_preselection_and_postselection_match_native_document_history() {
    for postselected in [false, true] {
        let mut gaps = Vec::new();
        for row in native::rows() {
            let value = &row["value"];
            let scene = native::setup(value);
            let mut app = test_app();
            app.document = scene.document;
            if postselected {
                app.document.clear_selection();
            }
            enter(&mut app, "MeshPolyline");
            if app.object_prompt.is_some() {
                app.select_prompt_objects(scene.sources.clone(), SelectionMode::Replace);
                enter(&mut app, "");
                if app.object_prompt.is_some() {
                    enter(&mut app, "Cancel");
                }
            }
            assert!(
                app.object_prompt.is_none(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            let check = native::check(
                &app.document,
                &scene.sources,
                &scene.parents,
                value,
                "after_script",
            );
            let qualified = check["qualified"] == true;
            if !qualified {
                gaps.push(row["id"].as_str().unwrap().to_owned());
            }
            if qualified && native::output_count(value) > 0 {
                assert_eq!(app.document.undo_label(), Some("MeshPolyline"));
                app.document.undo().unwrap();
                assert_eq!(
                    native::check(&app.document, &scene.sources, &scene.parents, value, "undo")["qualified"],
                    true
                );
                app.document.redo().unwrap();
                assert_eq!(
                    native::check(&app.document, &scene.sources, &scene.parents, value, "redo")["qualified"],
                    true
                );
            } else if native::output_count(value) == 0 {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "CLOSED_POLYLINE_{} {}",
                if postselected {
                    "POSTSELECTED"
                } else {
                    "PRESELECTED"
                },
                serde_json::json!({"id":row["id"],"qualified":qualified,"check":check,
                                  "grouped_history":qualified,"native_cancelled":native::cancelled(value)})
            );
        }
        assert_eq!(gaps, vec!["self_crossing"]);
    }
}

#[test]
fn cancelling_mesh_polyline_selection_preserves_redo() {
    let scene = native::setup(&native::rows()[0]["value"]);
    let mut app = test_app();
    app.document = scene.document;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    app.document.clear_selection();
    enter(&mut app, "MeshPolyline");
    app.select_prompt_objects(scene.sources, SelectionMode::Replace);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().count(), scene.parents.len());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert!(app.document.undo_label().is_none());
}
