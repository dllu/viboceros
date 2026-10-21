use super::*;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_box.rs"]
#[allow(dead_code)]
mod native;
use crate::app::subd_native_tests as subd_native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn native_box_getters_match_modes_source_selection_and_grouped_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["unsupported"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, &native::header(value));
        assert!(
            app.subd_primitive_session.is_some(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        for action in native::actions(value, scene.source) {
            let action = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if action == "Enter" { "" } else { &action });
            if app.subd_primitive_session.is_some() {
                assert_eq!(
                    app.document.objects().count(),
                    usize::from(scene.source.is_some())
                );
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_primitive_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDBox"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_BOX_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn count_options_cancel_preview_and_changed_history_keep_document_staged() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    enter(&mut app, "SubDBox");
    enter(&mut app, "XCount");
    enter(&mut app, "3");
    enter(&mut app, "YCount=2");
    enter(&mut app, "ZCount=1");
    for point in ["w0,0,0", "w4,3,0"] {
        enter(&mut app, point);
    }
    app.update_subd_primitive_hover(Point3::try_new(0., 0., 5.).unwrap());
    assert!(
        !app.subd_primitive_session
            .as_ref()
            .unwrap()
            .preview_curves()
            .is_empty()
    );
    app.update_subd_primitive_hover(Point3::try_new(0., 0., -5.).unwrap());
    let preview = app
        .subd_primitive_session
        .as_ref()
        .unwrap()
        .preview_curves();
    assert!(
        preview
            .iter()
            .any(|curve| curve.control_points().iter().any(|p| p.point().z() < -4.))
    );
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    app.cancel_interactive_command(true);
    assert_eq!(app.commands.subd_box_preferences().counts, [3, 2, 1]);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "SubDBox");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w4,3,0");
    enter(&mut app, "0");
    assert!(app.subd_primitive_session.is_some());
    assert_eq!(app.document.redo_label(), Some("Point"));
    app.document
        .add_geometry(Geometry::Point(Point3::try_new(2., 3., 4.).unwrap()))
        .unwrap();
    enter(&mut app, "5");
    assert!(app.subd_primitive_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}

use native as box_native;
#[path = "../../../crates/viboceros-command/tests/support/subd_plane.rs"]
mod plane_native;
#[test]
fn plane_getters_match_native_tags_and_complete_without_a_height_prompt() {
    for row in plane_native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["unsupported"] != true)
    {
        let value = &row["value"];
        let mut scene = plane_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(plane_native::plane(value));
        enter(&mut app, &plane_native::header(value));
        assert!(app.subd_primitive_session.is_some());
        for action in plane_native::actions(value, scene.source) {
            let action = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, &action);
            if app.subd_primitive_session.is_some() {
                assert_eq!(
                    app.document.objects().count(),
                    usize::from(scene.source.is_some())
                );
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_primitive_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = plane_native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDPlane"));
            scene.document.undo().unwrap();
            plane_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(plane_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_PLANE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn plane_counts_are_independent_and_cancellation_preserves_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    enter(&mut app, "SubDPlane");
    enter(&mut app, "XCount");
    enter(&mut app, "3");
    enter(&mut app, "YCount=4");
    enter(&mut app, "ZCount=5");
    assert!(app.subd_primitive_session.is_some());
    enter(&mut app, "w0,0,0");
    enter(&mut app, "Cancel");
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert_eq!(app.commands.subd_plane_preferences().counts, [3, 4]);
    assert_eq!(app.commands.subd_box_preferences().counts, [2; 3]);
    enter(&mut app, "SubDPlane");
    enter(&mut app, "w0,0,0");
    assert!(
        app.document
            .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap())
    );
    enter(&mut app, "w4,3,0");
    assert!(app.subd_primitive_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
}
#[test]
fn failed_plane_admission_ends_the_draft_without_consuming_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    enter(&mut app, "SubDPlane XCount=100000 YCount=100000");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w4,3,0");
    assert!(app.subd_primitive_session.is_none());
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
}
