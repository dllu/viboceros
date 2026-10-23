use super::*;
use crate::app::crease_native_tests::fixture as native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/rebuild_normals.rs"]
mod fixtures;

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn rebuild_normals_preselection_postselection_and_cancel_match_native_lifecycle() {
    for (family, rows) in fixtures::groups(true) {
        for row in rows {
            eprintln!("Rebuild normals getter {}", row["id"]);
            let value = &row["value"];
            let (doc, ids) = native::setup_from_family(&row, family);
            let mut app = test_app();
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
            enter(&mut app, "RebuildMeshNormals");
            if value["input"]["preselected"] == false {
                assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
                assert_eq!(app.document.undo_label(), Some("Undo witness"));
                if value["input"]["command_arguments"]
                    .as_str()
                    .is_some_and(|s| s.contains("_All"))
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
                enter(
                    &mut app,
                    if value["success"] == true {
                        ""
                    } else {
                        "Cancel"
                    },
                );
            }
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
            println!("REBUILD_NORMALS_GETTER {}", fixtures::report(&row, maximum));
        }
    }
}
