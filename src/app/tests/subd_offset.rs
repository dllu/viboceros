use super::*;
use crate::app::tests::subd_native as common;
#[path = "../../../crates/viboceros-oracle/tests/support/subd_offset.rs"]
mod support;
use viboceros_oracle::subd_join::snapshot;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn offset_subd_native_commands_replay_selection_options_previews_and_history() {
    let sources = support::sources();
    for (op, native) in support::cases_for("subd_offset_command") {
        let mut app = test_app();
        let (doc, id) = support::fixture(&sources[op["scene"].as_str().unwrap()]);
        app.document = doc;
        if op["preselect"] == true {
            app.document
                .select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "OffsetSubD");
        if op["preselect"] != true {
            assert!(app.object_prompt.is_some());
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        assert!(
            app.offset_subd_session.is_some(),
            "{op:?}: {:?}",
            app.command_log
        );
        assert!(!app.viewport_drafting_input(false, false).active);
        let options = support::options(&op);
        let command = options.command();
        let update = command.strip_prefix("OffsetSubD ").unwrap();
        enter(&mut app, update);
        assert!(!app.document.can_undo());
        assert!(!app.offset_subd_session.as_ref().unwrap().edges().is_empty());
        enter(&mut app, "");
        assert!(app.command_line_idle(), "{op:?}: {:?}", app.command_log);
        support::compare(
            &snapshot(&app.document, &[id]),
            &support::expected(&native["after"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
        enter(&mut app, "SelNone");
        support::compare(
            &snapshot(&app.document, &[id]),
            &support::expected(&native["after_script"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
        enter(&mut app, "Undo");
        support::compare(
            &snapshot(&app.document, &[id]),
            &support::expected(&native["undo"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
        enter(&mut app, "Redo");
        support::compare(
            &snapshot(&app.document, &[id]),
            &support::expected(&native["redo"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
    }
}
#[test]
fn offset_subd_native_option_defaults_and_cancel_lifetimes_match_application() {
    let sources = support::sources();
    let mut app = test_app();
    let native = support::cases_for("subd_offset_preferences").remove(0).1;
    for (i, row) in native.as_array().unwrap().iter().enumerate() {
        let (doc, id) = support::fixture(&sources["quad"]);
        app.document = doc;
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "OffsetSubD");
        if row["input"]["defaults"] != true {
            enter(
                &mut app,
                support::options(&row["input"])
                    .command()
                    .strip_prefix("OffsetSubD ")
                    .unwrap(),
            );
        }
        if row["input"]["cancel"] == true {
            app.cancel_interactive_command(true);
        } else {
            enter(&mut app, "");
        }
        support::compare(
            &snapshot(&app.document, &[id]),
            &support::expected(&row["value"]["after"]),
            &format!("preferences {i}"),
        );
    }
}
#[test]
fn offset_subd_flips_inputs_rejects_stale_sources_and_preserves_redo_on_cancel() {
    let sources = support::sources();
    let (doc, id) = support::fixture(&sources["quad"]);
    let mut app = test_app();
    app.document = doc;
    enter(&mut app, "Point 50,50,50");
    enter(&mut app, "Undo");
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "OffsetSubD");
    let before = app.document.object(id).unwrap().geometry_snapshot().clone();
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "Distance=0");
    assert!(app.offset_subd_session.is_some());
    assert_eq!(app.document.redo_label(), Some("Point"));
    app.cancel_interactive_command(true);
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert_eq!(
        app.document.object(id).unwrap().geometry_snapshot(),
        &before
    );
    enter(&mut app, "OffsetSubD");
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    let Geometry::SubD(s) = app.document.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(s.control_points()[0].to_array(), [0., 0., -1.]);
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "OffsetSubD");
    app.document
        .set_object_names([(id, Some("changed".into()))])
        .unwrap();
    let count = app.document.objects().len();
    enter(&mut app, "");
    assert_eq!(app.document.objects().len(), count);
    assert!(app.offset_subd_session.is_some());
    app.cancel_interactive_command(true);
}

#[test]
fn offset_subd_native_multiple_sources_keep_groups_and_output_order() {
    let sources = support::sources();
    for (op, native) in support::cases_for("subd_offset_batch") {
        let mut app = test_app();
        let (doc, ids) = support::fixture_many(&[&sources["quad"], &sources["closed"]]);
        app.document = doc;
        if op["preselect"] == true {
            app.document
                .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
                .unwrap();
        }
        enter(&mut app, "OffsetSubD");
        if op["preselect"] != true {
            for &id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Replace,
                });
            }
            enter(&mut app, "");
        }
        assert!(app.offset_subd_session.is_some(), "{:?}", app.command_log);
        let options = viboceros_command::offset_subd::Options {
            solid: op["solid"].as_bool().unwrap(),
            both_sides: op["both_sides"].as_bool().unwrap(),
            distance: 0.75,
            ..Default::default()
        };
        enter(
            &mut app,
            options.command().strip_prefix("OffsetSubD ").unwrap(),
        );
        enter(&mut app, "");
        support::compare(
            &snapshot(&app.document, &ids),
            &support::expected(&native["after"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
        enter(&mut app, "SelNone");
        enter(&mut app, "Undo");
        support::compare(
            &snapshot(&app.document, &ids),
            &support::expected(&native["undo"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
        enter(&mut app, "Redo");
        support::compare(
            &snapshot(&app.document, &ids),
            &support::expected(&native["redo"]),
            &format!(
                "{}:{}",
                op["id"].as_str().unwrap(),
                app.document.undo_label().unwrap_or("no history")
            ),
        );
    }
}

mod viewport;
