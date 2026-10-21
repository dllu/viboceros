use super::*;
use crate::app::subd_primitives::tests::native as box_native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_truncated_cone.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn truncated_cone_getters_match_native_caps_signed_axes_and_sizes() {
    let mut app = test_app();
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, "SubDTruncatedCone");
        for action in native::actions(value, scene.source) {
            let word = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if word == "Enter" { "" } else { &word });
            if app.subd_cylinder_session.is_some() {
                assert_eq!(app.active_command.unwrap().name(), "SubDTruncatedCone");
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
            assert_eq!(scene.document.undo_label(), Some("SubDTruncatedCone"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_TRUNCATED_CONE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn top_getter_freezes_axis_and_projects_picks_without_committing_drafts() {
    let mut app = test_app();
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    let cylinder = app.commands.subd_cylinder_preferences();
    let cone = app.commands.subd_cone_preferences();
    for action in [
        "SubDTruncatedCone AroundFaces=8 VerticalFaces=2",
        "w0,0,0",
        "3",
        "5",
    ] {
        enter(&mut app, action);
    }
    let plane = app.drafting_plane.unwrap();
    assert_eq!(plane.origin().to_array(), [0., 0., 5.]);
    assert_eq!(app.subd_cylinder_session.as_ref().unwrap().preview.len(), 5);
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "DirectionConstraint=None");
    assert!(app.subd_cylinder_session.is_some());
    assert_eq!(app.drafting_plane, Some(plane));
    app.update_subd_cylinder_hover(Point3::try_new(2., 3., 9.).unwrap());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    assert_eq!(app.commands.subd_cylinder_preferences(), cylinder);
    assert_eq!(app.commands.subd_cone_preferences(), cone);
    for action in ["SubDTruncatedCone", "w0,0,0", "3", "5", "Diameter", "2"] {
        enter(&mut app, action);
    }
    assert!(app.subd_cylinder_session.is_none());
    let prefs = app.commands.subd_truncated_cone_preferences();
    assert_eq!(prefs.end_size, 2.);
    assert!(prefs.diameter);
    assert_eq!(app.document.undo_label(), Some("SubDTruncatedCone"));
    for action in ["SubDTruncatedCone", "w0,0,0", "", "", "Radius", ""] {
        enter(&mut app, action);
    }
    assert!(app.subd_cylinder_session.is_none());
    let prefs = app.commands.subd_truncated_cone_preferences();
    assert_eq!(prefs.radius, 3.);
    assert_eq!(prefs.end_size, 2.);
    assert!(!prefs.diameter);
}
#[test]
fn top_getter_stale_history_and_tolerance_preserve_external_changes() {
    let mut app = test_app();
    for action in ["SubDTruncatedCone", "w0,0,0", "3", "5"] {
        enter(&mut app, action);
    }
    app.document
        .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap());
    enter(&mut app, "1");
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
    for action in ["SubDTruncatedCone", "w0,0,0", "3", "5"] {
        enter(&mut app, action);
    }
    app.document
        .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
        .unwrap();
    enter(&mut app, "1");
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}

use super::tests::references_native as cylinder_native;
#[path = "../../../crates/viboceros-command/tests/support/subd_truncated_cone_references.rs"]
#[allow(unused_imports)]
mod references_native;
#[test]
fn truncated_cone_reference_getters_match_native_tangencies_fit_grips_and_history() {
    for row in references_native::rows() {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "SubDTruncatedCone");
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
            assert_eq!(scene.document.undo_label(), Some("SubDTruncatedCone"));
            scene.document.undo().unwrap();
            references_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(references_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_TRUNCATED_CONE_REFERENCE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn truncated_perspective_pointer_resolves_the_top_plane_and_preserves_cplane() {
    use crate::viewport::{DraftingInput, ViewportInput, ViewportOutput};
    let mut app = test_app();
    app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Perspective);
    app.active_viewport = 0;
    enter(&mut app, "SubDTruncatedCone");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "3");
    enter(&mut app, "5");
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
    let target = Point3::try_new(2., 3., 5.).unwrap();
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
    let received = output.drafting_hover.expect("perspective top-radius hover");
    assert!(received.distance_to(target).unwrap() < 1e-4, "{received:?}");
    app.handle_viewport_action(output);
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(received),
        ..Default::default()
    }));
    assert!(app.subd_cylinder_session.is_none());
    assert_eq!(app.document.undo_label(), Some("SubDTruncatedCone"));
    assert_eq!(app.viewports[0].construction_plane(), cplane);
}

#[test]
fn fitted_grip_clicks_and_zero_base_cones_preserve_sources_and_grouped_history() {
    let row = references_native::rows()
        .into_iter()
        .find(|r| r["id"] == "truncated_fit_grips_postselected")
        .unwrap();
    let value = &row["value"];
    let mut scene = references_native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(
        &mut app,
        "SubDTruncatedCone Solid=Yes AroundFaces=8 CapFaceStyle=Quad VerticalFaces=2 FitPoints",
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
    assert!(app.subd_cylinder_session.is_some());
    enter(&mut app, "1");
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
        enter(
            &mut app,
            "SubDTruncatedCone Solid=Yes AroundFaces=8 CapFaceStyle=Quad VerticalFaces=2 FitPoints",
        );
        assert!(app.subd_cylinder_session.is_some());
        assert_eq!(app.document.redo_label(), Some("Point"));
        enter(&mut app, "5");
        assert!(app.subd_cylinder_session.is_some());
        enter(&mut app, "1");
        assert!(app.subd_cylinder_session.is_none());
        assert_eq!(app.document.redo_label(), None);
        assert_eq!(app.document.undo_label(), Some("SubDTruncatedCone"));
        std::mem::swap(&mut scene.document, &mut app.document);
        references_native::check(&scene, value, "after_script");
    }
}
