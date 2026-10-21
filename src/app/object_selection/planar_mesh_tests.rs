use super::*;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/planar_mesh.rs"]
mod native;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn planar_mesh_preselection_and_postselection_preserve_native_regions_and_history() {
    let controls = native::post_rows();
    for postselected in [false, true] {
        let mut geometry_gaps = Vec::new();
        for row in native::rows() {
            let target = if postselected {
                controls
                    .iter()
                    .find(|r| {
                        r["id"].as_str() == Some(&format!("post_{}", row["id"].as_str().unwrap()))
                    })
                    .unwrap_or(&row)
            } else {
                &row
            };
            let value = &target["value"];
            let scene = native::setup(value);
            let mut app = test_app();
            app.document = scene.document;
            if postselected {
                app.document.clear_selection();
            }
            enter(&mut app, "PlanarMesh");
            if app.object_prompt.is_some() {
                app.select_prompt_objects(scene.sources.clone(), SelectionMode::Replace);
                enter(&mut app, "");
            }
            assert!(
                app.object_prompt.is_none(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            let failed = app
                .command_log
                .iter()
                .any(|line| line.starts_with("Error:"));
            assert_eq!(
                failed,
                !native::native_success(value),
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
            if check["geometry_qualified"] != true {
                geometry_gaps.push(row["id"].clone());
            }
            if native::output_count(value) > 0 {
                assert_eq!(app.document.undo_label(), Some("PlanarMesh"));
                app.document.undo().unwrap();
                assert_eq!(
                    native::check(&app.document, &scene.sources, &scene.parents, value, "undo")["geometry_qualified"],
                    true
                );
                app.document.redo().unwrap();
                assert_eq!(
                    native::check(&app.document, &scene.sources, &scene.parents, value, "redo"),
                    check
                );
            } else {
                assert!(app.document.undo_label().is_none());
            }
            println!(
                "PLANAR_MESH_{} {}",
                if postselected {
                    "POSTSELECTED"
                } else {
                    "PRESELECTED"
                },
                serde_json::json!({
                "id":row["id"],"target_id":target["id"],"qualified":check["qualified"],"geometry_qualified":check["geometry_qualified"],
                "check":check,"native_failed":!native::native_success(value),"local_failed":failed,"native_status_match":true,"grouped_history":true})
            );
        }
        assert!(geometry_gaps.is_empty(), "{geometry_gaps:?}");
    }
}

#[test]
fn planar_mesh_postselection_controls_match_native_success_and_failure_selection() {
    for row in native::post_rows() {
        let value = &row["value"];
        let scene = native::setup(value);
        let mut app = test_app();
        app.document = scene.document;
        enter(&mut app, "PlanarMesh");
        app.select_prompt_objects(scene.sources.clone(), SelectionMode::Replace);
        enter(&mut app, "");
        assert!(
            app.object_prompt.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        let failed = app
            .command_log
            .iter()
            .any(|line| line.starts_with("Error:"));
        assert_eq!(failed, !native::native_success(value));
        let check = native::check(
            &app.document,
            &scene.sources,
            &scene.parents,
            value,
            "after_script",
        );
        assert_eq!(check["qualified"], true);
        if native::output_count(value) > 0 {
            app.document.undo().unwrap();
            assert_eq!(
                native::check(&app.document, &scene.sources, &scene.parents, value, "undo")["qualified"],
                true
            );
            app.document.redo().unwrap();
            assert_eq!(
                native::check(&app.document, &scene.sources, &scene.parents, value, "redo"),
                check
            );
        } else {
            assert!(app.document.undo_label().is_none());
        }
        println!(
            "PLANAR_MESH_POSTCONTROL {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"],"geometry_qualified":check["geometry_qualified"],
            "check":check,"native_failed":failed,"local_failed":failed,"native_status_match":true,"grouped_history":true})
        );
    }
}

#[test]
fn cancelling_planar_mesh_selection_preserves_redo_and_geometry() {
    let scene = native::setup(&native::rows()[0]["value"]);
    let mut app = test_app();
    app.document = scene.document;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    app.document.clear_selection();
    enter(&mut app, "PlanarMesh");
    app.select_prompt_objects(scene.sources, SelectionMode::Replace);
    enter(&mut app, "Cancel");
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().count(), scene.parents.len());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert!(app.document.undo_label().is_none());
}
