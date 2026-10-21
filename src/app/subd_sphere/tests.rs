use super::*;
use crate::app::subd_primitives::tests::native as box_native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_sphere.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn sphere_getters_match_native_styles_modes_source_selection_and_grouped_history() {
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, "SubDSphere");
        for action in native::actions(value, scene.source) {
            let word = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if word == "Enter" { "" } else { &word });
            if app.subd_sphere_session.is_some() {
                assert_eq!(
                    app.document.objects().count(),
                    usize::from(scene.source.is_some())
                );
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_sphere_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDSphere"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_SPHERE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

use crate::app::subd_axial::tests::references_native as cylinder_native;
#[path = "../../../crates/viboceros-command/tests/support/subd_sphere_references.rs"]
#[allow(unused_imports)]
mod references_native;
#[test]
fn sphere_reference_getters_match_tangent_fitted_and_guided_native_workflows() {
    for row in references_native::rows() {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "SubDSphere");
        for action in references_native::actions(value) {
            if action.is_object() {
                if action.get("pick").is_some() {
                    assert!(app.pick_tangent_at(
                        scene.sources[action["curve"].as_u64().unwrap() as usize],
                        None,
                        references_native::point(&action["pick"])
                    ));
                } else {
                    assert!(app.pick_circle_around_curve(
                        Some(scene.sources[action["curve"].as_u64().unwrap() as usize]),
                        None
                    ));
                }
            } else {
                let word = references_native::token(&action);
                let word = if action.is_array() {
                    format!("w{word}")
                } else {
                    word
                };
                enter(&mut app, if word == "Enter" { "" } else { &word });
            }
            if app.subd_sphere_session.is_some() {
                assert_eq!(app.document.objects().count(), scene.sources.len());
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_sphere_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = references_native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true && value["input"]["no_output"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDSphere"));
            scene.document.undo().unwrap();
            references_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(references_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_SPHERE_REFERENCE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn styles_preview_cancellation_and_invalid_topology_preserve_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    let cylinder = app.commands.subd_cylinder_preferences();
    enter(&mut app, "SubDSphere");
    for word in [
        "Subdivisions=2",
        "Style=Triangle",
        "Subdivisions=0",
        "Style=UV",
        "AroundFaces=7",
        "VerticalFaces=3",
        "Style=Quad",
    ] {
        enter(&mut app, word);
    }
    assert_eq!(
        app.subd_sphere_session
            .as_ref()
            .unwrap()
            .input
            .preferences
            .quad_subdivisions,
        2
    );
    enter(&mut app, "w0,0,0");
    app.update_subd_sphere_hover(Point3::try_new(3., 4., 0.).unwrap());
    assert_eq!(app.subd_sphere_session.as_ref().unwrap().preview.len(), 3);
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "-3");
    assert!(app.subd_sphere_session.is_some());
    enter(&mut app, "Cancel");
    let preferences = app.commands.subd_sphere_preferences();
    assert_eq!(preferences.triangle_subdivisions, 0);
    assert_eq!(
        (preferences.around_faces, preferences.vertical_faces),
        (7, 3)
    );
    assert_eq!(app.commands.subd_cylinder_preferences(), cylinder);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(
        &mut app,
        "SubDSphere Style=UV AroundFaces=100000 VerticalFaces=100000",
    );
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    assert!(app.subd_sphere_session.is_none());
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert_eq!(app.document.objects().count(), 0);
}

#[test]
fn first_pick_latches_viewport_plane_and_stale_drafts_are_cancelled() {
    let mut app = test_app();
    enter(&mut app, "SubDSphere 2Point");
    let frame = Frame3::try_from_directions(
        Point3::try_new(1., 2., 3.).unwrap(),
        viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(frame);
    app.accept_subd_sphere_point(frame.origin()).unwrap();
    app.viewports[app.active_viewport]
        .set_construction_plane(viboceros_command::CommandContext::default().construction_plane);
    let mut getter = app.subd_sphere_session.as_ref().unwrap().input;
    getter.point(frame.point_at([6., 0., 0.]).unwrap()).unwrap();
    assert_eq!(getter.definition.unwrap().frame.z_axis(), frame.z_axis());
    app.document
        .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap());
    app.accept_subd_sphere_point(frame.point_at([6., 0., 0.]).unwrap());
    assert!(app.subd_sphere_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "SubDSphere");
    enter(&mut app, "w0,0,0");
    app.document
        .add_geometry(Geometry::Point(frame.origin()))
        .unwrap();
    enter(&mut app, "3");
    assert!(app.subd_sphere_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn source_picker_handoffs_cancel_when_parent_history_changes() {
    for mode in ["Tangent", "AroundCurve", "FitPoints"] {
        let mut app = test_app();
        enter(&mut app, "Line 0,0,0 0,0,10");
        let id = app.document.objects().next().unwrap().id();
        app.document.clear_selection();
        enter(&mut app, &format!("SubDSphere {mode}"));
        if mode == "Tangent" {
            app.pick_tangent_at(id, None, Point3::try_new(0., 0., 3.).unwrap());
        }
        app.document.delete_objects([id]).unwrap();
        if mode == "Tangent" {
            app.pick_tangent_at(id, None, Point3::try_new(0., 0., 4.).unwrap());
        } else if mode == "AroundCurve" {
            app.pick_circle_around_curve(Some(id), None);
        } else {
            enter(&mut app, "");
        }
        assert!(app.subd_sphere_session.is_none(), "{mode}");
        assert!(app.tangent_session.is_none());
        assert!(app.circle_around_session.is_none());
        assert_eq!(app.document.objects().count(), 0);
    }
}

#[test]
fn fitted_grip_clicks_and_no_output_fits_preserve_source_and_history() {
    let row = references_native::rows()
        .into_iter()
        .find(|r| r["id"] == "sphere_fit_grips_postselected")
        .unwrap();
    let value = &row["value"];
    let mut scene = references_native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(&mut app, "SubDSphere FitPoints");
    assert!(app.control_point_picking_available());
    for index in 0..4 {
        assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
            control_point_selection: Some(crate::viewport::ControlPointSelection {
                picks: vec![viboceros_document::ControlPointId {
                    object: scene.sources[0],
                    index
                }],
                mode: viboceros_document::SelectionMode::Replace
            }),
            ..Default::default()
        }));
        assert_eq!(app.document.selected_control_points().count(), index + 1);
    }
    enter(&mut app, "");
    assert!(app.subd_sphere_session.is_none());
    std::mem::swap(&mut scene.document, &mut app.document);
    references_native::check(&scene, value, "after_script");
    for id in ["sphere_fit_collinear", "sphere_fit_collinear_postselected"] {
        let row = references_native::rows()
            .into_iter()
            .find(|r| r["id"] == id)
            .unwrap();
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "Point 99,99,99");
        app.document.undo().unwrap();
        enter(&mut app, "SubDSphere FitPoints");
        if app.object_prompt.is_some() {
            enter(&mut app, "SelAll");
            enter(&mut app, "");
        }
        assert!(app.subd_sphere_session.is_none());
        assert_eq!(app.document.redo_label(), Some("Point"));
        std::mem::swap(&mut scene.document, &mut app.document);
        references_native::check(&scene, value, "after_script");
    }
}
