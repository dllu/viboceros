use super::*;
use crate::app::subd_native_tests as subd_native;
use crate::app::subd_primitives::tests::native as box_native;
use crate::app::tests::test_app;
#[path = "../../../crates/viboceros-command/tests/support/subd_ellipsoid.rs"]
mod native;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command_input();
}
#[test]
fn native_getters_match_ellipsoid_modes_sources_and_grouped_history() {
    let mut app = test_app();
    for row in native::rows() {
        if row["fresh"] == true {
            app = test_app();
        }
        let value = &row["value"];
        let mut scene = native::setup(value);
        std::mem::swap(&mut scene.document, &mut app.document);
        app.viewports[app.active_viewport].set_construction_plane(native::plane(value));
        enter(&mut app, "SubDEllipsoid");
        for action in native::actions(value, scene.source) {
            let word = if action.contains(',') && !action.contains('=') {
                format!("w{action}")
            } else {
                action
            };
            enter(&mut app, if word == "Enter" { "" } else { &word });
            if app.subd_ellipsoid_session.is_some() {
                assert_eq!(
                    app.document.objects().count(),
                    usize::from(scene.source.is_some())
                );
                assert_eq!(app.document.undo_label(), None);
            }
        }
        assert!(
            app.subd_ellipsoid_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        assert!(app.ellipse_session.is_none(), "{}", row["id"]);
        std::mem::swap(&mut scene.document, &mut app.document);
        let check = native::check(&scene, value, "after_script");
        let saved = scene.document.objects().cloned().collect::<Vec<_>>();
        if value["input"]["cancel"] != true && value["input"]["no_output"] != true {
            assert_eq!(scene.document.undo_label(), Some("SubDEllipsoid"));
            scene.document.undo().unwrap();
            native::check(&scene, value, "undo");
            scene.document.redo().unwrap();
            native::check(&scene, value, "redo");
            assert_eq!(scene.document.objects().cloned().collect::<Vec<_>>(), saved);
        } else {
            assert_eq!(scene.document.undo_label(), None);
        }
        println!(
            "SUBD_ELLIPSOID_GETTER {}",
            serde_json::json!({"id":row["id"],"check":check,"grouped_history":true})
        );
    }
}
#[test]
fn pending_counts_preview_units_and_independent_settings_preserve_redo() {
    let mut app = test_app();
    enter(&mut app, "Point 99,99,99");
    app.document.undo().unwrap();
    let ellipse = app.commands.ellipse_options();
    let torus = app.commands.subd_torus_preferences();
    enter(&mut app, "SubDEllipsoid AroundFaces");
    assert_eq!(
        app.accept_subd_ellipsoid_point(Point3::try_new(0., 0., 0.).unwrap()),
        Some(false)
    );
    for word in [
        "8",
        "VerticalFaces=4",
        "CapFaceStyle=Tri",
        "0",
        "5mm",
        "w3,4,0",
        "3mm",
        "w0,0,9",
    ] {
        enter(&mut app, word);
    }
    assert!(app.ellipse_session.is_none());
    let plane = app.drafting_plane;
    let hover = Point3::try_new(1.6, -1.2, 0.).unwrap();
    app.update_subd_ellipsoid_hover(hover);
    assert_eq!(
        app.subd_ellipsoid_session.as_ref().unwrap().preview.len(),
        3
    );
    for word in ["AroundFaces=10", "Radius", "Deformable"] {
        enter(&mut app, word);
        assert!(app.subd_ellipsoid_session.is_some());
    }
    assert_eq!(app.drafting_plane, plane);
    assert_eq!(app.document.objects().len(), 0);
    assert_eq!(app.document.redo_label(), Some("Point"));
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Point"));
    for word in ["SubDEllipsoid", "0", "w5,0,0", "w0,3,0", "2cm"] {
        enter(&mut app, word);
    }
    assert!(app.subd_ellipsoid_session.is_none());
    assert_eq!(app.document.undo_label(), Some("SubDEllipsoid"));
    let Geometry::SubD(s) = app.document.objects().next().unwrap().geometry() else {
        panic!("SubD")
    };
    assert!(
        (s.surface_points()
            .iter()
            .map(|p| p.z().abs())
            .fold(0., f64::max)
            - 20.)
            .abs()
            < 1e-10
    );
    assert_eq!(app.commands.ellipse_options(), ellipse);
    assert_eq!(app.commands.subd_torus_preferences(), torus);
    let saved = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.undo().unwrap();
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), saved);
}
#[test]
fn stale_history_tolerance_and_guide_edits_discard_both_getters() {
    for third in [false, true] {
        let mut app = test_app();
        for word in ["SubDEllipsoid", "w0,0,0"] {
            enter(&mut app, word);
        }
        if third {
            enter(&mut app, "w5,0,0");
            enter(&mut app, "w0,3,0");
        }
        app.document
            .set_tolerance(Tolerance::try_new(1e-4, 1e-8, 1e-6).unwrap());
        enter(&mut app, "2");
        assert!(app.subd_ellipsoid_session.is_none() && app.ellipse_session.is_none());
        assert_eq!(app.document.objects().len(), 0);
    }
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 0,0,10");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "SubDEllipsoid AroundCurve");
    assert!(app.pick_ellipse_curve(Some(id), None));
    app.document
        .begin_transaction("External source deletion")
        .unwrap();
    app.document.delete_objects([id]).unwrap();
    app.document.commit_transaction().unwrap();
    app.validate_ellipse_curve();
    assert!(app.subd_ellipsoid_session.is_none() && app.ellipse_session.is_none());
    assert_eq!(app.document.objects().len(), 0);
    let mut app = test_app();
    enter(&mut app, "SubDEllipsoid");
    app.document
        .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
        .unwrap();
    enter(&mut app, "w0,0,0");
    assert!(app.subd_ellipsoid_session.is_none() && app.ellipse_session.is_none());
    assert_eq!(app.document.objects().len(), 1);
}
#[test]
fn focal_completion_keeps_independent_marking_and_a_single_history_record() {
    let mut app = test_app();
    let ellipse = app.commands.ellipse_options();
    for word in [
        "SubDEllipsoid AroundFaces=8 VerticalFaces=4 CapFaceStyle=Tri FromFoci",
        "w-4,0,0",
        "w4,0,0",
        "MarkFoci=Yes",
        "w0,3,0",
    ] {
        enter(&mut app, word);
    }
    assert!(app.subd_ellipsoid_session.is_none() && app.ellipse_session.is_none());
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), Some("SubDEllipsoid"));
    assert!(app.commands.subd_ellipsoid_preferences().mark_foci);
    assert_eq!(app.commands.ellipse_options(), ellipse);
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 0);
    app.document.redo().unwrap();
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn ellipsoid_perspective_pointer_resolves_the_third_plane_and_preserves_cplane() {
    use crate::viewport::{DraftingInput, ViewportInput, ViewportOutput};
    let mut app = test_app();
    app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Perspective);
    app.active_viewport = 0;
    enter(&mut app, "SubDEllipsoid");
    enter(&mut app, "w0,0,0");
    enter(&mut app, "w5,0,0");
    enter(&mut app, "w0,3,0");
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
    let target = Point3::try_new(0., 0., 2.).unwrap();
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
    let received = output.drafting_hover.expect("perspective third-axis hover");
    assert!(received.distance_to(target).unwrap() < 1e-4, "{received:?}");
    app.handle_viewport_action(output);
    assert_eq!(app.document.objects().count(), 0);
    assert!(app.handle_viewport_action(ViewportOutput {
        picked_point: Some(received),
        ..Default::default()
    }));
    assert!(app.subd_ellipsoid_session.is_none());
    assert_eq!(app.document.undo_label(), Some("SubDEllipsoid"));
    assert_eq!(app.viewports[0].construction_plane(), cplane);
}
