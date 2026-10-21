use super::*;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/face_create.rs"]
mod native;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}

#[test]
fn face_point_getters_match_native_geometry_ngons_and_identity_through_history() {
    let mut gaps = Vec::new();
    for row in native::rows() {
        let value = &row["value"];
        let scene = native::setup(value);
        let mut app = test_app();
        app.document = scene.document;
        enter(&mut app, &native::header(value));
        assert!(app.face_create_session.is_some());
        for action in native::actions(value) {
            enter(&mut app, &action);
        }
        if app.face_create_session.is_some() {
            enter(&mut app, "");
        }
        assert!(
            app.face_create_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        assert!(app.active_command.is_none());
        let check = native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        if check["qualified"] != true {
            gaps.push(row["id"].clone());
        }
        assert_eq!(app.document.undo_label(), Some("3DFace"));
        app.document.undo().unwrap();
        assert_eq!(
            native::check(
                &app.document,
                scene.source,
                scene.original.as_ref(),
                value,
                "undo"
            )["qualified"],
            true
        );
        app.document.redo().unwrap();
        assert_eq!(
            native::check(
                &app.document,
                scene.source,
                scene.original.as_ref(),
                value,
                "redo"
            ),
            check
        );
        println!(
            "FACE_CREATE_GETTER {}",
            serde_json::json!({"id":row["id"],"qualified":check["qualified"],"check":check,"grouped_history":true})
        );
    }
    assert!(gaps.is_empty(), "{gaps:?}");
}

#[test]
fn complete_script_and_partial_cancel_keep_redo_and_preview_drafts_separate() {
    let rows = native::rows();
    let value = &rows[0]["value"];
    let scene = native::setup(value);
    let mut app = test_app();
    app.document = scene.document;
    app.commands
        .execute(&mut app.document, "Point 9,9,9")
        .unwrap();
    app.document.undo().unwrap();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "3DFace");
    enter(&mut app, "0,0,0");
    enter(&mut app, "4,0,0");
    app.update_face_create_hover(Point3::try_new(4., 3., 0.).unwrap());
    assert!(
        !app.face_create_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .is_empty()
    );
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    app.cancel_current_prompt_or_selection();
    assert!(
        app.face_create_session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .points
            .is_empty()
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, &native::command(value));
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(
        native::check(&app.document, None, None, value, "after_script")["qualified"],
        true
    );
}

#[test]
fn appending_from_a_boundary_edge_uses_reversed_face_winding_and_one_group() {
    let rows = native::rows();
    let value = &rows.iter().find(|r| r["id"] == "append_flat").unwrap()["value"];
    let scene = native::setup(value);
    let id = scene.source.unwrap();
    let mut app = test_app();
    app.document = scene.document;
    let Geometry::Mesh(mesh) = app.document.object(id).unwrap().geometry() else {
        panic!("mesh")
    };
    let edge = mesh
        .diagnostic_edges()
        .iter()
        .find(|e| e.points.iter().all(|p| p.x() == 4.))
        .unwrap()
        .index;
    enter(&mut app, "3DFace Output=Mesh FromEdge=Yes Mode=SingleFace");
    assert!(app.picking_face_edge());
    assert!(app.pick_face_edge_click(&[crate::viewport::EdgePick { object: id, edge }]));
    enter(&mut app, "7,0,0");
    enter(&mut app, "7,3,0");
    assert!(app.face_create_session.is_none());
    assert_eq!(
        native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script"
        )["qualified"],
        true
    );
    app.document.undo().unwrap();
    assert_eq!(
        native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "undo"
        )["qualified"],
        true
    );
}

#[test]
fn stale_source_or_history_discards_the_draft_without_overwriting_new_work() {
    let scene = native::setup(&native::rows()[0]["value"]);
    let mut app = test_app();
    app.document = scene.document;
    enter(&mut app, "3DFace");
    enter(&mut app, "0,0,0");
    app.commands
        .execute(&mut app.document, "Point 5,5,5")
        .unwrap();
    enter(&mut app, "4,0,0");
    assert!(app.face_create_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.undo_label(), Some("Point"));
}

#[test]
fn changing_options_preserves_corners_and_append_id_selects_the_requested_mesh() {
    let rows = native::rows();
    let value = &rows.iter().find(|r| r["id"] == "append_flat").unwrap()["value"];
    let scene = native::setup(value);
    let id = scene.source.unwrap();
    let mut app = test_app();
    app.document = scene.document;
    app.document.clear_selection();
    enter(&mut app, "3DFace");
    enter(&mut app, &format!("Append={id}"));
    assert_eq!(app.face_create_session.as_ref().unwrap().target, Some(id));
    enter(&mut app, "4,0,0");
    enter(&mut app, "WeldAngle=0");
    assert_eq!(
        app.face_create_session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .points
            .len(),
        1
    );
    enter(&mut app, "FromEdge=Yes");
    assert!(
        !app.face_create_session
            .as_ref()
            .unwrap()
            .preferences
            .from_edge
    );
    assert_eq!(
        app.face_create_session
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .points
            .len(),
        1
    );
    for point in ["7,0,0", "7,3,0", "4,3,0"] {
        enter(&mut app, point);
    }
    assert!(app.face_create_session.is_none());
    assert_eq!(
        native::check(
            &app.document,
            Some(id),
            scene.original.as_ref(),
            value,
            "after_script"
        )["qualified"],
        true
    );
}

#[test]
fn physical_native_edge_and_escape_controls_match_document_and_history() {
    for row in native::controls() {
        let value = &row["value"];
        let scene = native::setup(value);
        let mut app = test_app();
        app.document = scene.document;
        app.commands
            .execute(&mut app.document, "Point 99,99,99")
            .unwrap();
        app.document.undo().unwrap();
        if let Some(id) = scene.source {
            app.document
                .select_objects_direct([id], viboceros_document::SelectionMode::Replace)
                .unwrap();
        }
        let from_edge = value["input"]["edge_pick"].is_array();
        let header = if from_edge {
            native::header(value).replace("FromEdge=No", "FromEdge=Yes")
        } else {
            native::header(value)
        };
        enter(&mut app, &header);
        if from_edge {
            let id = scene.source.unwrap();
            let Geometry::Mesh(mesh) = app.document.object(id).unwrap().geometry() else {
                panic!("mesh")
            };
            let edge = mesh
                .diagnostic_edges()
                .iter()
                .find(|e| e.points.iter().all(|p| p.x() == 4.))
                .unwrap()
                .index;
            assert!(app.pick_face_edge_click(&[crate::viewport::EdgePick { object: id, edge }]));
        }
        for action in native::actions(value) {
            enter(&mut app, &action);
        }
        if value["input"]["cancel"] == true {
            for _ in 0..3 {
                if app.face_create_session.is_none() {
                    break;
                }
                app.cancel_current_prompt_or_selection();
            }
        } else if app.face_create_session.is_some() {
            enter(&mut app, "");
        }
        assert!(
            app.face_create_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        let check = native::check(
            &app.document,
            scene.source,
            scene.original.as_ref(),
            value,
            "after_script",
        );
        assert_eq!(check["qualified"], true, "{} {check}", row["id"]);
        if app.document.undo_label().is_some() {
            assert_eq!(app.document.undo_label(), Some("3DFace"));
            app.document.undo().unwrap();
            assert_eq!(
                native::check(
                    &app.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "undo"
                )["qualified"],
                true
            );
            app.document.redo().unwrap();
            assert_eq!(
                native::check(
                    &app.document,
                    scene.source,
                    scene.original.as_ref(),
                    value,
                    "redo"
                ),
                check
            );
        } else {
            assert_eq!(app.document.redo_label(), Some("Point"));
        }
        println!(
            "FACE_CREATE_CONTROLS {}",
            serde_json::json!({"id":row["id"],"qualified":true,"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn in_plane_hover_matches_the_projected_corner_before_admission() {
    let mut app = test_app();
    enter(&mut app, "3DFace InPlane=Yes");
    for point in ["0,0,0", "4,0,0", "4,3,2"] {
        enter(&mut app, point);
    }
    app.update_face_create_hover(Point3::try_new(0., 3., 0.).unwrap());
    let expected = Point3::try_new(0., 27. / 13., 18. / 13.).unwrap();
    let preview = app.face_create_session.as_ref().unwrap().preview_curves();
    assert!(
        preview[0]
            .control_points()
            .iter()
            .any(|p| p.point().distance_to(expected).unwrap() < 1e-12)
    );
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "0,3,0");
    let Geometry::Mesh(mesh) = app.document.objects().next().unwrap().geometry() else {
        panic!("mesh")
    };
    assert!(mesh.vertices()[3].distance_to(expected).unwrap() < 1e-12);
}
