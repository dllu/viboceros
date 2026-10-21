use super::*;
use crate::app::subd_primitives::tests::native as box_native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_cone.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn cone_getters_match_native_caps_signed_axes_sizes_and_angles() {
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, "SubDCone");
        for action in native::actions(value, scene.source) {
            let word = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if word == "Enter" { "" } else { &word });
            if app.subd_cylinder_session.is_some() {
                assert_eq!(app.active_command.unwrap().name(), "SubDCone");
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
            assert_eq!(scene.document.undo_label(), Some("SubDCone"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_CONE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn independent_defaults_count_retry_previews_and_stale_drafts_preserve_history() {
    let mut app = test_app();
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    let cylinder = app.commands.subd_cylinder_preferences();
    enter(&mut app, "SubDCone");
    assert_eq!(
        app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .preferences
            .height,
        10.
    );
    enter(&mut app, "AroundFaces");
    enter(&mut app, "-3");
    assert_eq!(app.commands.subd_cone_preferences().around_faces, 8);
    enter(&mut app, "7");
    enter(&mut app, "VerticalFaces=3");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    app.update_subd_cylinder_hover(Point3::try_new(0., 0., 5.).unwrap());
    assert_eq!(app.subd_cylinder_session.as_ref().unwrap().preview.len(), 4);
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert_eq!(app.commands.subd_cone_preferences().around_faces, 7);
    assert_eq!(app.commands.subd_cylinder_preferences(), cylinder);
    enter(&mut app, "SubDCone 2Point");
    let frame = Frame3::try_from_directions(
        Point3::try_new(1., 2., 3.).unwrap(),
        viboceros_geometry::Vector3::try_new(1., 0., 0.).unwrap(),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[app.active_viewport].set_construction_plane(frame);
    app.accept_subd_cylinder_point(frame.origin()).unwrap();
    app.accept_subd_cylinder_point(frame.point_at([6., 0., 0.]).unwrap())
        .unwrap();
    assert_eq!(
        app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .circle
            .unwrap()
            .frame
            .z_axis(),
        frame.z_axis()
    );
    app.document
        .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap());
    let redo_after_external_change = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "5");
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(
        app.document.redo_label(),
        redo_after_external_change.as_deref()
    );
    enter(&mut app, "SubDCone");
    enter(&mut app, "w0,0,0");
    app.document
        .add_geometry(Geometry::Point(frame.origin()))
        .unwrap();
    enter(&mut app, "3");
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}
#[test]
fn angle_defaults_units_and_radius_height_memory_use_the_shared_controller() {
    let mut app = test_app();
    enter(&mut app, "SubDCone");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    enter(&mut app, "OrBaseAngle");
    enter(&mut app, "-30");
    assert!(app.subd_cylinder_session.is_none());
    let prefs = app.commands.subd_cone_preferences();
    assert_eq!(prefs.base_angle, -30.);
    assert!((prefs.height + 3_f64.sqrt()).abs() < 1e-12);
    assert_eq!(app.document.undo_label(), Some("SubDCone"));
    enter(&mut app, "SubDCone");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    enter(&mut app, "OrApexAngle=180");
    assert!(app.subd_cylinder_session.is_some());
    enter(&mut app, "OrApexAngle");
    enter(&mut app, "");
    assert!(app.subd_cylinder_session.is_none());
    assert!((app.commands.subd_cone_preferences().height - 3. * 3_f64.sqrt()).abs() < 1e-12);
    enter(&mut app, "SubDCone");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "2cm");
    assert_eq!(
        app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .circle
            .unwrap()
            .radius,
        20.
    );
    enter(&mut app, "Cancel");
    assert_eq!(app.commands.subd_cylinder_preferences().radius, 1.);
}

use super::tests::references_native as cylinder_native;
#[path = "../../../crates/viboceros-command/tests/support/subd_cone_references.rs"]
#[allow(unused_imports)]
mod references_native;
#[test]
fn cone_reference_getters_match_native_tangencies_fit_grips_and_history() {
    for row in references_native::rows() {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "SubDCone");
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
            assert_eq!(scene.document.undo_label(), Some("SubDCone"));
            scene.document.undo().unwrap();
            references_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(references_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_CONE_REFERENCE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn stale_tangent_sources_and_unsupported_tangent_angles_leave_the_draft_atomic() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 12,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    enter(&mut app, "SubDCone Tangent");
    app.pick_tangent_at(id, None, Point3::try_new(3., 0., 0.).unwrap());
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "SubDCone Tangent");
    app.pick_tangent_at(id, None, Point3::try_new(3., 0., 0.).unwrap());
    app.document.delete_objects([id]).unwrap();
    app.pick_tangent_at(id, None, Point3::try_new(4., 0., 0.).unwrap());
    assert!(app.subd_cylinder_session.is_none());
    assert!(app.tangent_session.is_none());
    enter(&mut app, "Line 0,0,0 12,0,0");
    enter(&mut app, "Line 0,0,0 0,12,0");
    let sources = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document.clear_selection();
    enter(&mut app, "SubDCone Tangent");
    app.pick_tangent_at(sources[0], None, Point3::try_new(3., 0., 0.).unwrap());
    app.pick_tangent_at(sources[1], None, Point3::try_new(0., 4., 0.).unwrap());
    enter(&mut app, "");
    enter(&mut app, "OrBaseAngle=30");
    assert!(
        app.subd_cylinder_session
            .as_ref()
            .unwrap()
            .input
            .definition
            .is_none()
    );
    assert_eq!(app.document.objects().count(), 2);
    enter(&mut app, "Cancel");
    for text in [
        "SubDCone Tangent",
        "Point",
        "w3,0,0",
        "Point",
        "w0,3,0",
        "Point",
        "w-3,0,0",
        "OrBaseAngle=30",
    ] {
        enter(&mut app, text);
    }
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.objects().count(), 3);
    assert!((app.commands.subd_cone_preferences().height - 3_f64.sqrt()).abs() < 1e-12);
}

#[test]
fn fitted_grip_clicks_and_degenerate_cones_preserve_source_and_pending_redo() {
    let row = references_native::rows()
        .into_iter()
        .find(|r| r["id"] == "cone_fit_grips_postselected")
        .unwrap();
    let value = &row["value"];
    let mut scene = references_native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(
        &mut app,
        "SubDCone Solid=Yes AroundFaces=8 CapFaceStyle=Quad VerticalFaces=2 FitPoints",
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
    enter(&mut app, "5");
    assert!(app.subd_cylinder_session.is_none());
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
        enter(&mut app, "SubDCone FitPoints");
        assert!(app.subd_cylinder_session.is_some());
        assert_eq!(app.document.redo_label(), Some("Point"));
        enter(&mut app, "5");
        assert!(app.subd_cylinder_session.is_none());
        assert_eq!(app.document.redo_label(), Some("Point"));
        std::mem::swap(&mut scene.document, &mut app.document);
        references_native::check(&scene, value, "after_script");
    }
}

#[test]
fn cone_perspective_pointer_resolves_the_height_plane_and_preserves_cplane() {
    use crate::viewport::{DraftingInput, ViewportInput, ViewportOutput};
    let mut app = test_app();
    app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Perspective);
    app.active_viewport = 0;
    enter(&mut app, "SubDCone");
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
    assert_eq!(app.document.undo_label(), Some("SubDCone"));
    assert_eq!(app.viewports[0].construction_plane(), cplane);
}
