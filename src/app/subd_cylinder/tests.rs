use super::*;
use crate::app::subd_primitives::tests::native as box_native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_cylinder.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn native_getters_match_radius_modes_caps_axes_and_grouped_history() {
    for row in native::rows()
        .into_iter()
        .filter(|r| r["value"]["input"]["unsupported"] != true)
    {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, "SubDCylinder");
        assert!(app.subd_cylinder_session.is_some());
        for action in native::actions(value, scene.source) {
            let action = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if action == "Enter" { "" } else { &action });
            if app.subd_cylinder_session.is_some() {
                assert_eq!(app.document.objects().count(), 0);
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_cylinder_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDCylinder"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_CYLINDER_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn counts_preview_cancel_retry_and_stale_history_preserve_staged_document() {
    let mut app = test_app();
    enter(&mut app, "Point 9,9,9");
    app.document.undo().unwrap();
    enter(&mut app, "SubDCylinder");
    enter(&mut app, "AroundFaces");
    enter(&mut app, "7");
    enter(&mut app, "VerticalFaces=2");
    enter(&mut app, "Solid=No");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    enter(&mut app, "DirectionConstraint=None");
    app.update_subd_cylinder_hover(Point3::try_new(3., 4., 6.).unwrap());
    assert_eq!(app.subd_cylinder_session.as_ref().unwrap().preview.len(), 3);
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "0");
    assert!(app.subd_cylinder_session.is_some());
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert_eq!(app.commands.subd_cylinder_preferences().around_faces, 7);
    assert_eq!(app.commands.subd_box_preferences().counts, [2; 3]);
    enter(&mut app, "SubDCylinder");
    enter(&mut app, "w0,0,0");
    app.document
        .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
        .unwrap();
    enter(&mut app, "3");
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}
#[test]
fn viewport_plane_is_latched_at_first_pick_and_tolerance_changes_cancel() {
    let mut app = test_app();
    enter(&mut app, "SubDCylinder 2Point");
    let frame = Frame3::try_from_directions(
        Point3::try_new(1., 2., 3.).unwrap(),
        viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(frame);
    assert!(app.accept_subd_cylinder_point(frame.origin()).unwrap());
    assert!(
        app.accept_subd_cylinder_point(frame.point_at([6., 0., 0.]).unwrap())
            .unwrap()
    );
    let circle = app
        .subd_cylinder_session
        .as_ref()
        .unwrap()
        .input
        .circle
        .unwrap();
    assert_eq!(circle.frame.z_axis(), frame.z_axis());
    assert!(
        app.document
            .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap())
    );
    enter(&mut app, "5");
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
}

#[test]
fn numeric_free_axis_height_uses_hover_without_editing_the_document_preview() {
    let row = native::rows()
        .into_iter()
        .find(|r| r["id"] == "subd_cylinder_free_axis")
        .unwrap();
    let value = &row["value"];
    let mut scene = native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(
        &mut app,
        "SubDCylinder AroundFaces=8 VerticalFaces=2 CapFaceStyle=Quad",
    );
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    enter(&mut app, "DirectionConstraint=None");
    app.update_subd_cylinder_hover(Point3::try_new(3., 4., 6.).unwrap());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, &61_f64.sqrt().to_string());
    assert!(app.subd_cylinder_session.is_none());
    std::mem::swap(&mut scene.document, &mut app.document);
    native::check(&scene, value, "after_script");
}

#[test]
fn perspective_pointer_resolves_the_height_plane_and_preserves_cplane() {
    use crate::viewport::{DraftingInput, ViewportInput, ViewportOutput};
    let mut app = test_app();
    app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Perspective);
    app.active_viewport = 0;
    enter(&mut app, "SubDCylinder");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    let plane = app.drafting_plane.unwrap();
    let cplane = app.viewports[0].construction_plane();
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let mut rect = egui::Rect::NOTHING;
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ui| {
            rect = ui.available_rect_before_wrap();
            app.viewports[0].show(ui, &app.document, ViewportInput::default(), &[], 0, true);
        },
    )
    .drop_without_applying_deltas();
    let target = Point3::try_new(0., 0., 5.).unwrap();
    let pointer = app.viewports[0].project(target, rect).unwrap();
    let mut output = ViewportOutput::default();
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(screen),
            events: vec![egui::Event::PointerMoved(pointer)],
            ..Default::default()
        },
        |ui| {
            output = app.viewports[0].show(
                ui,
                &app.document,
                ViewportInput {
                    drafting: DraftingInput {
                        active: true,
                        anchor: app.active_command.and_then(InteractiveCommand::anchor),
                        ..Default::default()
                    },
                    angle_plane: Some(plane),
                    ..Default::default()
                },
                &[],
                0,
                true,
            );
        },
    )
    .drop_without_applying_deltas();
    let received = output.drafting_hover.expect("perspective height hover");
    assert!(received.distance_to(target).unwrap() < 1e-4, "{received:?}");
    app.handle_viewport_action(output);
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(received),
        ..Default::default()
    }));
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.undo_label(), Some("SubDCylinder"));
    assert_eq!(app.viewports[0].construction_plane(), cplane);
}

#[test]
fn filters_constraints_and_dimensionless_area_follow_the_common_point_tools() {
    let mut app = test_app();
    for word in ["SubDCylinder 2Point", "w0,0,0", "6"] {
        enter(&mut app, word);
    }
    assert!(app.point_constraint.is_some());
    enter(&mut app, "w3,4,0");
    assert!(
        (app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .circle
            .unwrap()
            .radius
            - 3.)
            .abs()
            < 1e-12
    );
    assert!(app.point_constraint.is_none());
    enter(&mut app, "Cancel");
    for word in ["SubDCylinder", ".xy", "w1,2,99", "w99,99,3", "Area", "9mm"] {
        enter(&mut app, word);
    }
    assert_eq!(
        app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .anchor()
            .unwrap()
            .to_array(),
        [1., 2., 3.]
    );
    assert!(
        app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .circle
            .is_none()
    );
    enter(&mut app, "9*pi");
    assert!(
        (app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .circle
            .unwrap()
            .radius
            - 3.)
            .abs()
            < 1e-12
    );
    enter(&mut app, ".x");
    assert!(app.point_filter.is_some());
    app.document
        .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap());
    enter(&mut app, "5");
    assert!(app.subd_cylinder_session.is_none());
    assert!(app.point_filter.is_none());
    assert!(app.point_constraint.is_none());
    assert_eq!(app.document.objects().count(), 0);
}

use crate::app::subd_native_tests as subd_native;
#[path = "../../../crates/viboceros-command/tests/support/subd_cylinder_references.rs"]
pub(in crate::app) mod references_native;
#[test]
fn native_reference_getters_match_contacts_fit_selection_grips_and_grouped_history() {
    for row in references_native::rows() {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        references_native::check(&scene, value, "before");
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "SubDCylinder");
        for action in references_native::actions(value) {
            if action.is_object() {
                assert!(app.pick_tangent_at(
                    scene.sources[action["curve"].as_u64().unwrap() as usize],
                    None,
                    references_native::point(&action["pick"])
                ));
            } else {
                let word = references_native::token(&action);
                let word = if action.is_array() {
                    format!("w{word}")
                } else {
                    word
                };
                enter(&mut app, if word == "Enter" { "" } else { &word });
            }
            if action == "FitPoints" && value["input"]["no_output"] == true {
                assert_eq!(
                    app.subd_cylinder_session
                        .as_ref()
                        .unwrap()
                        .input
                        .circle
                        .unwrap()
                        .radius,
                    0.
                );
                assert!(app.active_command.is_some());
            }
            if app.subd_cylinder_session.is_some() {
                assert_eq!(app.document.objects().count(), scene.sources.len());
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_cylinder_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = references_native::check(&scene, value, "after_script");
        if value["success"] == true {
            assert_eq!(scene.document.undo_label(), Some("SubDCylinder"));
            scene.document.undo().unwrap();
            references_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(references_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_CYLINDER_REFERENCE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn inline_reference_modes_and_stale_tangent_sources_keep_the_draft_atomic() {
    for name in [
        "cylinder_tangent_points",
        "cylinder_fit_preselected",
        "cylinder_fit_postselected",
    ] {
        let row = references_native::rows()
            .into_iter()
            .find(|r| r["id"] == name)
            .unwrap();
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        let actions = references_native::actions(value);
        let header = format!(
            "SubDCylinder {}",
            actions[..5]
                .iter()
                .map(references_native::token)
                .collect::<Vec<_>>()
                .join(" ")
        );
        enter(&mut app, &header);
        for a in &actions[5..] {
            let word = references_native::token(a);
            let word = if a.is_array() {
                format!("w{word}")
            } else {
                word
            };
            enter(&mut app, if word == "Enter" { "" } else { &word });
        }
        assert!(
            app.subd_cylinder_session.is_none(),
            "{name} {:?}",
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        references_native::check(&scene, value, "after_script");
    }
    let row = references_native::rows()
        .into_iter()
        .find(|r| r["id"] == "cylinder_tangent_two")
        .unwrap();
    let mut scene = references_native::setup(&row["value"]);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    enter(&mut app, "SubDCylinder Tangent");
    assert!(app.pick_tangent_at(scene.sources[0], None, Point3::try_new(3., 0., 0.).unwrap()));
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "SubDCylinder Tangent");
    assert!(app.pick_tangent_at(scene.sources[0], None, Point3::try_new(3., 0., 0.).unwrap()));
    app.document.delete_objects([scene.sources[0]]).unwrap();
    assert!(app.pick_tangent_at(scene.sources[1], None, Point3::try_new(0., 4., 0.).unwrap()));
    assert!(app.subd_cylinder_session.is_none());
    assert!(app.tangent_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn fitted_grip_clicks_and_degenerate_fits_preserve_pending_redo() {
    let row = references_native::rows()
        .into_iter()
        .find(|r| r["id"] == "cylinder_fit_grips_postselected")
        .unwrap();
    let value = &row["value"];
    let mut scene = references_native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    enter(
        &mut app,
        "SubDCylinder Solid=Yes AroundFaces=8 CapFaceStyle=Quad VerticalFaces=2 FitPoints",
    );
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
    assert!(app.object_prompt.is_none());
    assert_eq!(app.document.objects().count(), 1);
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert!((app.commands.subd_cylinder_preferences().radius - 2.).abs() < 1e-12);
    assert_eq!(app.commands.circle_preferences().radius, 1.);
    enter(&mut app, "5");
    std::mem::swap(&mut scene.document, &mut app.document);
    references_native::check(&scene, value, "after_script");
    for row in references_native::rows()
        .into_iter()
        .filter(|r| r["id"].as_str().unwrap().ends_with("_finish"))
    {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "Point 99,99,99");
        app.document.undo().unwrap();
        enter(&mut app, "SubDCylinder FitPoints");
        assert_eq!(
            app.subd_cylinder_session
                .as_ref()
                .unwrap()
                .input
                .circle
                .unwrap()
                .radius,
            0.
        );
        assert_eq!(app.document.redo_label(), Some("Point"));
        enter(&mut app, "5");
        assert!(app.subd_cylinder_session.is_none());
        assert_eq!(app.document.redo_label(), Some("Point"));
        std::mem::swap(&mut scene.document, &mut app.document);
        references_native::check(&scene, value, "after_script");
    }
}
