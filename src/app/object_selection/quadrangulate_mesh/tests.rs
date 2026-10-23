use super::*;
use crate::app::tests::test_app;
use crate::app::{subd_native_tests as subd_native, subdivision_native_tests as mesh_native};
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-command/tests/support/quad_command.rs"]
mod fixtures;
use crate::app::crease_native_tests::fixture as native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn quadrangulate_mesh_getters_match_native_geometry_preferences_cancellation_and_history() {
    for (family, rows) in fixtures::groups() {
        let mut app = test_app();
        for row in rows {
            eprintln!("Getter {}", row["id"]);
            let value = &row["value"];
            let (doc, ids) = native::setup_from_family(&row, family);
            app.document = doc;
            let originals = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            let defaults = app.commands.quadrangulate_mesh_options(&[]).unwrap();
            enter(&mut app, "QuadrangulateMesh");
            assert!(
                app.object_prompt
                    .as_ref()
                    .unwrap()
                    .quadrangulate_mesh
                    .is_some()
            );
            if value["input"]["preselected"] == false && value["input"]["cancel"] != true {
                app.select_prompt_objects(ids.iter().copied(), SelectionMode::Replace);
                enter(&mut app, "");
                assert_eq!(
                    app.object_prompt.as_ref().unwrap().phase,
                    ObjectPromptPhase::Options
                );
            }
            let args = fixtures::arguments(value);
            if !args.is_empty() {
                enter(&mut app, &args);
            }
            for (id, original) in ids.iter().zip(&originals) {
                assert_eq!(
                    app.document.object(*id).unwrap().geometry_snapshot(),
                    original
                );
            }
            assert_eq!(app.document.undo_label(), Some("Undo witness"));
            enter(
                &mut app,
                if value["success"] == true {
                    ""
                } else {
                    "Cancel"
                },
            );
            assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
            let mut maximum = fixtures::check(&app.document, &ids, value, "after");
            if value["success"] != true {
                assert_eq!(
                    app.commands.quadrangulate_mesh_options(&[]).unwrap(),
                    defaults
                );
            }
            app.document.undo().unwrap();
            maximum = maximum.max(fixtures::check(&app.document, &ids, value, "undo"));
            app.document.redo().unwrap();
            maximum = maximum.max(fixtures::check(&app.document, &ids, value, "redo"));
            println!(
                "QUAD_GETTER {}",
                fixtures::report(
                    &row,
                    app.commands.quadrangulate_mesh_options(&[]).unwrap(),
                    maximum
                )
            );
        }
    }
}

#[test]
fn quadrangulate_mesh_numeric_input_is_atomic_and_cancel_keeps_redo() {
    let (family, rows) = fixtures::groups().remove(0);
    let (doc, ids) = native::setup_from_family(&rows[0], family);
    let mut app = test_app();
    app.document = doc;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    let defaults = app.commands.quadrangulate_mesh_options(&[]).unwrap();
    enter(&mut app, "QuadrangulateMesh");
    enter(&mut app, "Planarity=30 Rectangularity=0");
    assert_eq!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .quadrangulate_mesh
            .as_ref()
            .unwrap()
            .options,
        defaults
    );
    enter(&mut app, "Planarity");
    enter(&mut app, "NaN");
    assert_eq!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .quadrangulate_mesh
            .as_ref()
            .unwrap()
            .value,
        Some("Planarity")
    );
    enter(&mut app, "30");
    enter(&mut app, "Rectangularity");
    enter(&mut app, "3");
    assert_eq!(
        app.commands.quadrangulate_mesh_options(&[]).unwrap(),
        defaults
    );
    assert_eq!(app.document.redo_label(), redo.as_deref());
    app.cancel_interactive_command(true);
    assert!(app.document.selected_object_ids().next().is_none());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(
        app.commands.quadrangulate_mesh_options(&[]).unwrap(),
        defaults
    );
    assert!(app.document.object(ids[0]).is_some());
}

#[test]
fn quadrangulate_mesh_stale_options_reject_changed_sources_and_tolerance() {
    for tolerance_change in [false, true] {
        let (family, rows) = fixtures::groups().remove(0);
        let (doc, ids) = native::setup_from_family(&rows[0], family);
        let mut app = test_app();
        app.document = doc;
        enter(&mut app, "QuadrangulateMesh");
        if tolerance_change {
            app.document
                .set_tolerance(Tolerance::try_new(1e-4, 1e-12, 1e-6).unwrap());
        } else {
            app.document
                .replace_object_geometries([(
                    ids[0],
                    Geometry::Point(viboceros_geometry::Point3::try_new(0., 0., 0.).unwrap()),
                )])
                .unwrap();
        }
        let undo = app.document.undo_label().map(str::to_owned);
        enter(&mut app, "");
        assert!(app.object_prompt.is_none());
        assert_eq!(app.document.undo_label(), undo.as_deref());
        assert_eq!(
            app.commands.quadrangulate_mesh_options(&[]).unwrap(),
            Options::default()
        );
    }
}
