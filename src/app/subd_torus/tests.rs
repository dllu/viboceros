#[path = "../../../crates/viboceros-command/tests/support/subd_torus_surface.rs"]
mod torus_surface;
use super::*;
use crate::app::subd_primitives::tests::native as box_native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_torus.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn torus_getters_match_native_frames_sizes_and_inner_dimensions() {
    let mut app = test_app();
    for row in native::rows() {
        let value = &row["value"];
        let mut scene = native::setup(value);
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, "SubDTorus");
        for action in native::actions(value, scene.source) {
            let word = if action.contains(',') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if word == "Enter" { "" } else { &word });
            if app.subd_torus_session.is_some() {
                assert_eq!(app.active_command.unwrap().name(), "SubDTorus");
                assert_eq!(
                    app.document.objects().count(),
                    usize::from(scene.source.is_some())
                );
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_torus_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDTorus"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_TORUS_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

use crate::app::subd_axial::tests::references_native as cylinder_native;
#[path = "../../../crates/viboceros-command/tests/support/subd_torus_references.rs"]
#[allow(unused_imports)]
mod references_native;
#[test]
fn torus_reference_getters_match_tangent_fitted_and_guided_native_workflows() {
    for row in references_native::rows() {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "SubDTorus");
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
            if app.subd_torus_session.is_some() {
                assert_eq!(app.document.objects().count(), scene.sources.len());
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_torus_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = references_native::check(&scene, value, "after_script");
        if value["input"]["cancel"] != true && value["input"]["no_output"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDTorus"));
            scene.document.undo().unwrap();
            references_native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            assert_eq!(references_native::check(&scene, value, "redo"), check);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_TORUS_REFERENCE_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}

#[test]
fn tube_retry_assigned_units_and_independent_settings_preserve_pending_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    let cylinder = app.commands.subd_cylinder_preferences();
    let sphere = app.commands.subd_sphere_preferences();
    for action in ["SubDTorus AroundFaces=8 VerticalFaces=6", "w0,0,0", "3"] {
        enter(&mut app, action);
    }
    assert_eq!(app.subd_torus_session.as_ref().unwrap().preview.len(), 4);
    let plane = app.drafting_plane;
    for action in ["4", "AroundFaces=12", "Area=9"] {
        enter(&mut app, action);
        assert!(app.subd_torus_session.is_some());
    }
    assert_eq!(app.drafting_plane, plane);
    assert_eq!(app.document.objects().count(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    app.update_subd_torus_hover(Point3::try_new(4., 0., 9.).unwrap());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    for action in [
        "SubDTorus",
        "w0,0,0",
        "3",
        "FixInnerDimension=Yes",
        "Diameter=2cm",
    ] {
        enter(&mut app, action);
    }
    assert!(app.subd_torus_session.is_none());
    assert_eq!(app.document.undo_label(), Some("SubDTorus"));
    let prefs = app.commands.subd_torus_preferences();
    assert_eq!(prefs.radius, 3.);
    assert_eq!(prefs.tube_radius, 10.);
    assert!(prefs.tube_diameter && prefs.fix_inner_dimension);
    assert!(!prefs.diameter);
    assert_eq!(app.commands.subd_cylinder_preferences(), cylinder);
    assert_eq!(app.commands.subd_sphere_preferences(), sphere);
    let Geometry::SubD(subd) = app.document.objects().next().unwrap().geometry() else {
        panic!("SubD torus")
    };
    let radii = subd
        .surface_points()
        .iter()
        .map(|p| p.x().hypot(p.y()))
        .collect::<Vec<_>>();
    assert!((radii.iter().copied().fold(0_f64, f64::max) - 23.).abs() < 1e-10);
    assert!((radii.iter().copied().fold(f64::INFINITY, f64::min) - 3.).abs() < 1e-10);
    app.document.undo().unwrap();
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().count(), 1);
}
#[test]
fn stale_history_tolerance_and_tangent_sources_cancel_torus_drafts() {
    let mut app = test_app();
    for action in ["SubDTorus", "w0,0,0", "3"] {
        enter(&mut app, action);
    }
    app.document
        .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap());
    enter(&mut app, "1");
    assert!(app.subd_torus_session.is_none());
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "Line 0,0,0 12,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "SubDTorus Tangent");
    app.pick_tangent_at(id, None, Point3::try_new(3., 0., 0.).unwrap());
    app.document.delete_objects([id]).unwrap();
    app.pick_tangent_at(id, None, Point3::try_new(4., 0., 0.).unwrap());
    assert!(app.subd_torus_session.is_none());
    assert!(app.tangent_session.is_none());
    for action in ["SubDTorus", "w0,0,0", "3"] {
        enter(&mut app, action);
    }
    app.document
        .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
        .unwrap();
    enter(&mut app, "1");
    assert!(app.subd_torus_session.is_none());
    assert_eq!(app.document.objects().count(), 1);
}
#[test]
fn selected_grip_events_and_failed_zero_bases_preserve_sources_and_redo() {
    let row = references_native::rows()
        .into_iter()
        .find(|r| r["id"] == "torus_fit_grips_postselected")
        .unwrap();
    let value = &row["value"];
    let mut scene = references_native::setup(value);
    let mut app = test_app();
    std::mem::swap(&mut scene.document, &mut app.document);
    enter(
        &mut app,
        "SubDTorus AroundFaces=8 VerticalFaces=6 FitPoints",
    );
    assert!(app.control_point_picking_available());
    for index in 0..4 {
        assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
            control_point_selection: Some(crate::viewport::ControlPointSelection {
                picks: vec![viboceros_document::ControlPointId {
                    object: scene.sources[0],
                    index
                }],
                mode: viboceros_document::SelectionMode::Replace,
            }),
            ..Default::default()
        }));
        assert_eq!(app.document.selected_control_points().count(), index + 1);
    }
    enter(&mut app, "");
    assert!(app.object_prompt.is_none());
    assert!(app.subd_torus_session.is_some());
    enter(&mut app, "1");
    assert!(app.subd_torus_session.is_none());
    std::mem::swap(&mut scene.document, &mut app.document);
    references_native::check(&scene, value, "after_script");
    for row in references_native::rows()
        .into_iter()
        .filter(|r| r["id"].as_str().unwrap().ends_with("_fixed"))
    {
        let value = &row["value"];
        let mut scene = references_native::setup(value);
        let mut app = test_app();
        std::mem::swap(&mut scene.document, &mut app.document);
        enter(&mut app, "Point 99,99,99");
        app.document.undo().unwrap();
        enter(
            &mut app,
            "SubDTorus AroundFaces=8 VerticalFaces=6 FitPoints",
        );
        assert!(app.subd_torus_session.is_some());
        enter(&mut app, "1");
        assert!(app.subd_torus_session.is_some());
        enter(&mut app, "FixInnerDimension=Yes");
        enter(&mut app, "1");
        assert!(app.subd_torus_session.is_none());
        assert_eq!(app.document.redo_label(), Some("Point"));
        std::mem::swap(&mut scene.document, &mut app.document);
        references_native::check(&scene, value, "after_script");
    }
}

#[test]
fn torus_perspective_pointer_resolves_the_tube_plane_and_preserves_cplane() {
    use crate::viewport::{DraftingInput, ViewportInput, ViewportOutput};
    let mut app = test_app();
    app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Perspective);
    app.active_viewport = 0;
    enter(&mut app, "SubDTorus");
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
    let target = Point3::try_new(4., 0., 0.).unwrap();
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
    let received = output
        .drafting_hover
        .expect("perspective tube-radius hover");
    assert!(received.distance_to(target).unwrap() < 1e-4, "{received:?}");
    app.handle_viewport_action(output);
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(received),
        ..Default::default()
    }));
    assert!(app.subd_torus_session.is_none());
    assert_eq!(app.document.undo_label(), Some("SubDTorus"));
    assert_eq!(app.viewports[0].construction_plane(), cplane);
}
