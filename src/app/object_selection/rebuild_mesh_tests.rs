use super::*;
use crate::app::crease_native_tests::fixture as native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/rebuild_mesh.rs"]
mod fixtures;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn rebuild_mesh_options_selection_cancellation_and_preferences_match_native_workflows() {
    for (family, rows) in fixtures::groups() {
        let mut app = test_app();
        for row in rows {
            eprintln!("Mesh rebuild getter {}", row["id"]);
            let v = &row["value"];
            let before = app.commands.rebuild_mesh_options(&[]).unwrap();
            let staged = fixtures::staged(v, before);
            let (doc, ids) = native::setup_from_family(&row, family);
            app.document = doc;
            let snapshots = ids
                .iter()
                .map(|id| {
                    app.document
                        .object(*id)
                        .unwrap()
                        .geometry_snapshot()
                        .clone()
                })
                .collect::<Vec<_>>();
            enter(&mut app, "RebuildMesh");
            if v["input"]["preselected"] == false {
                assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
                for argument in fixtures::arguments(v) {
                    enter(&mut app, argument);
                }
                assert_eq!(app.commands.rebuild_mesh_options(&[]).unwrap(), before);
                let options = &app.object_prompt.as_ref().unwrap().description.options;
                assert_eq!(
                    options
                        .iter()
                        .find(|o| o.name == "PreserveTextureCoordinates")
                        .unwrap()
                        .value,
                    staged.preserve_texture_coordinates
                );
                assert_eq!(
                    options
                        .iter()
                        .find(|o| o.name == "PreserveVertexColors")
                        .unwrap()
                        .value,
                    staged.preserve_vertex_colors
                );
                if v["input"]["command_arguments"]
                    .as_str()
                    .unwrap_or("")
                    .contains("_All")
                {
                    app.select_prompt_objects(ids.iter().copied(), SelectionMode::Replace);
                }
                for (id, snapshot) in ids.iter().zip(&snapshots) {
                    assert!(
                        app.document
                            .object(*id)
                            .unwrap()
                            .geometry_snapshot()
                            .shares_storage_with(snapshot)
                    );
                }
                assert_eq!(app.document.undo_label(), Some("Undo witness"));
                enter(&mut app, if v["success"] == true { "" } else { "Cancel" });
            }
            assert!(app.object_prompt.is_none(), "{:?}", app.command_log);
            let actual = app.commands.rebuild_mesh_options(&[]).unwrap();
            assert_eq!(actual, if v["success"] == true { staged } else { before });
            let mut maximum = native::mesh_command::check(&app.document, &ids, v, "after");
            app.document.undo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&app.document, &ids, v, "undo"));
            assert_eq!(app.commands.rebuild_mesh_options(&[]).unwrap(), actual);
            app.document.redo().unwrap();
            maximum = maximum.max(native::mesh_command::check(&app.document, &ids, v, "redo"));
            assert_eq!(app.commands.rebuild_mesh_options(&[]).unwrap(), actual);
            println!(
                "REBUILD_MESH_GETTER {}",
                fixtures::report(&row, actual, maximum)
            );
        }
    }
}
