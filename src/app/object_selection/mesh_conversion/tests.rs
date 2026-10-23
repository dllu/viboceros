use super::*;
use crate::app::crease_native_tests::fixture as native;
use crate::app::tests::test_app;
#[path = "../../../../crates/viboceros-command/tests/support/nonplanar_command.rs"]
mod fixtures;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn nonplanar_getters_match_native_command_geometry_defaults_selection_and_history() {
    for (family, rows) in fixtures::groups() {
        let mut app = test_app();
        for row in rows {
            eprintln!("Nonplanar getter {}", row["id"]);
            let value = &row["value"];
            let (doc, ids) = native::setup_from_family(&row, family);
            app.document = doc;
            let sources = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            enter(&mut app, "TriangulateNonPlanarQuads");
            assert!(
                app.object_prompt
                    .as_ref()
                    .unwrap()
                    .mesh_conversion
                    .is_some()
            );
            if value["input"]["preselected"] == false && value["input"]["cancel"] != true {
                app.select_prompt_objects(ids.iter().copied(), SelectionMode::Replace);
                enter(&mut app, "");
            }
            let arguments = fixtures::arguments(value);
            if !arguments.is_empty() {
                enter(&mut app, &arguments)
            }
            if row["id"] == "nonplanar_command_split_menu" {
                enter(&mut app, "");
            }
            for (id, source) in ids.iter().zip(&sources) {
                assert!(
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .shares_storage_with(source)
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
            let mut maximum = native::mesh_command::check(&app.document, &ids, value, "after");
            app.document.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(
                &app.document,
                &ids,
                value,
                "undo",
            ));
            app.document.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(
                &app.document,
                &ids,
                value,
                "redo",
            ));
            println!(
                "NONPLANAR_GETTER {}",
                fixtures::report(
                    &row,
                    app.commands
                        .triangulate_nonplanar_quads_options(&[])
                        .unwrap(),
                    maximum
                )
            );
        }
    }
}

#[test]
fn nonplanar_options_are_staged_and_invalid_or_cancelled_edits_preserve_redo() {
    let (family, rows) = fixtures::groups().remove(0);
    let (doc, ids) = native::setup_from_family(&rows[0], family);
    let mut app = test_app();
    app.document = doc;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    let defaults = app
        .commands
        .triangulate_nonplanar_quads_options(&[])
        .unwrap();
    let source = app
        .document
        .object(ids[0])
        .unwrap()
        .geometry_snapshot()
        .clone();
    enter(&mut app, "TriangulateNonPlanarQuads");
    enter(&mut app, "SelectBy=Angle Distance=1 Angle=NaN");
    assert_eq!(
        app.object_prompt
            .as_ref()
            .unwrap()
            .mesh_conversion
            .as_ref()
            .unwrap()
            .options,
        Settings::Nonplanar(defaults)
    );
    enter(&mut app, "Mode");
    enter(&mut app, "Both");
    enter(&mut app, "Distance");
    enter(&mut app, "0.2");
    enter(&mut app, "SplitMethod");
    enter(&mut app, "LongestDiagonal");
    assert_eq!(
        app.commands
            .triangulate_nonplanar_quads_options(&[])
            .unwrap(),
        defaults
    );
    assert_eq!(
        app.document.object(ids[0]).unwrap().geometry_snapshot(),
        &source
    );
    enter(&mut app, "Cancel");
    assert!(app.document.selected_object_ids().next().is_none());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(
        app.commands
            .triangulate_nonplanar_quads_options(&[])
            .unwrap(),
        defaults
    );
}

#[test]
fn nonplanar_stale_sources_and_tolerance_reject_acceptance() {
    for tolerance_change in [false, true] {
        let (family, rows) = fixtures::groups().remove(0);
        let (doc, ids) = native::setup_from_family(&rows[0], family);
        let mut app = test_app();
        app.document = doc;
        enter(&mut app, "TriangulateNonPlanarQuads");
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
            app.commands
                .triangulate_nonplanar_quads_options(&[])
                .unwrap(),
            nonplanar::Options::default()
        );
    }
}
