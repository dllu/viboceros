use super::native::fixture as native;
use super::*;
use crate::app::{subd_native_tests as subd_native, subdivision_native_tests as mesh_native};
use crate::viewport::{SelectionClick, ViewportOutput};
use viboceros_command::insert_point::InsertAction;
#[allow(dead_code)]
#[path = "../../../../crates/viboceros-command/tests/support/insert_point.rs"]
mod fixtures;
#[test]
fn insertion_getter_replays_native_points_local_undo_finish_and_cancel() {
    for row in fixtures::rows() {
        replay(&row, "insert_point_full", "GETTER");
    }
}
#[test]
fn ngon_getter_preserves_native_overlays_and_document_history() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/insert_point_ngons.json"
    ))
    .unwrap();
    for row in capture["results"].as_array().unwrap() {
        replay(row, "insert_point_ngons", "NGON_GETTER");
    }
}
fn replay(row: &serde_json::Value, family: &str, stage: &str) {
    let v = &row["value"];
    eprintln!("Getter {}", row["id"]);
    let (doc, ids) = native::setup_from_family(row, family);
    let mut app = test_app();
    app.document = doc;
    let original = app.document.object(ids[0]).unwrap().geometry().clone();
    enter(&mut app, "InsertPoint");
    assert!(
        app.insert_prompt().unwrap().session.is_some(),
        "{:?}",
        app.command_log
    );
    for action in fixtures::actions(v) {
        match action {
            InsertAction::Point(point) => {
                let pick = app
                    .insert_prompt()
                    .unwrap()
                    .session
                    .as_ref()
                    .unwrap()
                    .pick_point(point)
                    .unwrap();
                app.handle_viewport_action(ViewportOutput {
                    insert_point: Some((
                        pick,
                        viboceros_geometry::Vector3::try_new(0., 0., 1.)
                            .unwrap()
                            .normalized_nonzero()
                            .unwrap(),
                    )),
                    ..Default::default()
                });
            }
            InsertAction::Undo => enter(&mut app, "Undo"),
            InsertAction::Finish => enter(&mut app, ""),
            InsertAction::Cancel => enter(&mut app, "Cancel"),
            _ => panic!(),
        }
        if let Some(prompt) = app.insert_prompt() {
            assert_eq!(app.document.object(ids[0]).unwrap().geometry(), &original);
            assert_eq!(app.document.undo_label(), Some("Undo witness"));
            assert_eq!(
                prompt.scene.object(ids[0]).unwrap().geometry(),
                prompt.session.as_ref().unwrap().candidate()
            );
        }
    }
    assert!(app.insert_prompt().is_none(), "{:?}", app.command_log);
    let comparison = fixtures::check(&app.document, &ids, &v["after"], true);
    if stage == "GETTER" {
        fixtures::assert_known_cell_gap(row["id"].as_str().unwrap(), &comparison);
    } else {
        fixtures::assert_ngon_cell_gap(row["id"].as_str().unwrap(), &comparison);
    }
    let changed = app.document.undo_label() == Some("InsertPoint");
    enter(&mut app, "Undo");
    fixtures::check(&app.document, &ids, &v["undo"], false);
    enter(&mut app, "Redo");
    fixtures::check(&app.document, &ids, &v["redo"], false);
    println!(
        "INSERT_POINT_{stage} {}",
        serde_json::json!({"id":row["id"],"comparison":comparison,"identity":true,"metadata":true,"history":true,"selection":true,"preview":true,"changed":changed})
    );
}
#[test]
fn command_first_selection_filters_sources_and_uses_the_same_point_getter() {
    let (mut app, id) = setup();
    let other = app
        .document
        .add_geometry(Geometry::Point(p(10., 10., 10.)))
        .unwrap();
    enter(&mut app, "InsertPoint");
    assert!(app.selecting_insert_source());
    app.apply_selection_click(SelectionClick {
        object_id: Some(other),
        mode: SelectionMode::Replace,
    });
    assert!(app.selecting_insert_source());
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    assert!(!app.selecting_insert_source());
    enter(&mut app, "0,1.5,0");
    assert_eq!(
        app.insert_prompt()
            .unwrap()
            .session
            .as_ref()
            .unwrap()
            .point_count(),
        1
    );
    enter(&mut app, "");
    assert_eq!(app.document.undo_label(), Some("InsertPoint"));
}
#[test]
fn escape_new_commands_and_undo_all_keep_source_geometry_and_redo() {
    for ending in [
        "Cancel",
        "_Cancel",
        "-Cancel",
        "_-Cancel",
        "Escape",
        "Point 8,9,10",
        "undo_all",
    ] {
        let (mut app, id) = setup();
        enter(&mut app, "Point 20,20,20");
        enter(&mut app, "Undo");
        let redo = app.document.redo_label().map(str::to_owned);
        let original = app.document.object(id).unwrap().geometry().clone();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "InsertPoint");
        enter(&mut app, "0,1.5,0");
        if ending == "undo_all" {
            enter(&mut app, "Undo");
            enter(&mut app, "");
        } else if ending == "Escape" {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, ending);
        }
        assert!(app.insert_prompt().is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        if ending != "Point 8,9,10" {
            assert_eq!(app.document.redo_label(), redo.as_deref());
        }
    }
}
#[test]
fn stale_sources_discard_previews_and_bad_typed_lines_stage_no_partial_points() {
    let (mut app, id) = setup();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "InsertPoint");
    enter(&mut app, "0,1.5,0 Edge=999999 Parameter=0.5");
    assert_eq!(
        app.insert_prompt()
            .unwrap()
            .session
            .as_ref()
            .unwrap()
            .point_count(),
        0
    );
    enter(&mut app, "0,1.5,0");
    app.commands
        .execute(&mut app.document, "Tolerance Absolute=0.02")
        .unwrap();
    enter(&mut app, "Undo");
    assert!(app.insert_prompt().is_none());
}
#[test]
fn negative_coordinates_pick_the_negative_side_without_losing_their_sign() {
    let (mut app, id) = setup();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "InsertPoint");
    enter(&mut app, "-100,1.5,0");
    let prompt = app.insert_prompt().unwrap();
    let Geometry::SubD(s) = prompt.session.as_ref().unwrap().candidate() else {
        panic!()
    };
    assert_eq!(s.control_points().len(), 7);
    assert!(
        s.control_points()
            .iter()
            .any(|p| p.x() == 0. && p.y() == 1.5)
    );
}
#[test]
fn prefixed_undo_remains_local_and_prefixed_enter_finishes() {
    for undo in ["Undo", "_Undo", "-Undo", "_-Undo"] {
        let (mut app, id) = setup();
        let original = app.document.object(id).unwrap().geometry().clone();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        enter(&mut app, "InsertPoint");
        enter(&mut app, "-100,1.5,0");
        enter(&mut app, undo);
        assert_eq!(
            app.insert_prompt()
                .unwrap()
                .session
                .as_ref()
                .unwrap()
                .point_count(),
            0
        );
        assert_eq!(app.document.object(id).unwrap().geometry(), &original);
        assert!(app.document.undo_label().is_none());
        enter(&mut app, "_-Enter");
        assert!(app.insert_prompt().is_none());
        assert!(app.document.undo_label().is_none());
    }
}
