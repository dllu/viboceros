use super::subd_native as support;
use super::*;
use viboceros_oracle::subd_join::{Fixture, expected, snapshot};
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn subd_join_native_commands_options_picks_and_history_match_application() {
    for (op, native) in support::cases_for("subd_join_history") {
        let fixture: Fixture = serde_json::from_value(native["input"].clone()).unwrap();
        let (doc, ids) = fixture.document();
        let mut app = test_app();
        app.document = doc;
        let id = op["id"].as_str().unwrap();
        let pre = op["preselect"].as_bool().unwrap();
        if pre {
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        support::compare(
            &snapshot(&app.document, &ids),
            &expected(&native["before"]),
            id,
        );
        enter(
            &mut app,
            &format!(
                "{} SubDJoinedEdges={}",
                op["command"].as_str().unwrap(),
                if fixture.crease { "Crease" } else { "Smooth" }
            ),
        );
        if !pre {
            assert!(app.object_prompt.is_some(), "{id}: {:?}", app.command_log);
            for &key in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(key),
                    mode: SelectionMode::Replace,
                });
                assert!(app.object_prompt.is_some(), "{id}: premature completion");
            }
            enter(&mut app, "");
        }
        assert!(app.object_prompt.is_none(), "{id}: {:?}", app.command_log);
        support::compare(
            &snapshot(&app.document, &ids),
            &expected(&native["after"]),
            id,
        );
        // Explicitly model RunScript's separate postselection cleanup; the
        // command-completion snapshot above is compared before this action.
        if !pre {
            enter(&mut app, "SelNone");
        }
        support::compare(
            &snapshot(&app.document, &ids),
            &expected(&native["after_script"]),
            &format!("{id} script return"),
        );
        enter(&mut app, "Undo");
        support::compare(
            &snapshot(&app.document, &ids),
            &expected(&native["undo"]),
            &format!("{id} undo"),
        );
        enter(&mut app, "Redo");
        support::compare(
            &snapshot(&app.document, &ids),
            &expected(&native["redo"]),
            &format!("{id} redo"),
        );
    }
}
