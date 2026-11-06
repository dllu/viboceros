use super::plane_surface_native_support as native;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/point_grid_input.rs"]
mod replay;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn surface_point_grid_getters_match_native_geometry_preferences_and_atomic_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("grid.3dm");
    let mut count = 0;
    for family in [
        "point_grid_input",
        "point_grid_input_holdouts",
        "point_grid_input_prompts",
    ] {
        let data = native::read(family);
        let mut app = test_app();
        for row in data["results"].as_array().unwrap() {
            if row["id"] == "control_separate_degree" {
                continue;
            } // Frozen driver-timeout diagnostic.
            let v = &row["value"];
            let trace = &v["input_trace"][0];
            assert_eq!(trace["completed"], trace["expected"]);
            assert!(trace["errors"].as_array().unwrap().is_empty());
            app.document = native::setup(v);
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            if v["success"] != true {
                app.commands
                    .execute(&mut app.document, "Point 100,200,300")
                    .unwrap();
                app.document.undo().unwrap();
            }
            enter(&mut app, v["command"].as_str().unwrap());
            for step in trace["sent"].as_array().unwrap() {
                let token = step["token"].as_str().unwrap().trim_start_matches('_');
                enter(&mut app, if token == "Enter" { "" } else { token });
            }
            assert!(
                app.surface_point_grid_session.is_none(),
                "{} {:?}",
                row["id"],
                app.command_log
            );
            assert!(app.active_command.is_none());
            assert!(app.drafting_plane.is_none());
            let mut error = replay::check(&app.document, v, "after", false);
            let mut archives = 0;
            if v["success"] == true {
                assert_eq!(app.document.undo_label(), v["command"].as_str());
                app.document.undo().unwrap();
                error = error.max(replay::check(&app.document, v, "undo", false));
                assert!(app.document.undo().unwrap().is_none());
                app.document.redo().unwrap();
                error = error.max(replay::check(&app.document, v, "redo", false));
                app.commands
                    .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                    .unwrap();
                let mut restored = Document::new(app.document.tolerance());
                app.commands
                    .execute(&mut restored, &format!("Open3dm {}", path.display()))
                    .unwrap();
                // Current-layer choice is document state, retained by 3DM.
                error = error.max(replay::check(&restored, v, "after", true));
                archives = 1;
            } else {
                assert!(!app.document.can_undo());
                assert!(app.document.can_redo());
            }
            for parent in before {
                assert_eq!(app.document.object(parent.id()), Some(&parent));
                assert!(
                    parent.geometry_snapshot().shares_storage_with(
                        app.document
                            .object(parent.id())
                            .unwrap()
                            .geometry_snapshot()
                    )
                );
            }
            println!(
                "POINT_GRID_UI {}",
                serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error,"archives":archives})
            );
            assert!(error <= 1e-6, "{} {error:e}", row["id"]);
            count += 1;
        }
    }
    assert_eq!(count, 24);
}

#[test]
fn surface_point_grid_drafts_reject_stale_documents_and_keep_point_constraints_local() {
    let mut app = test_app();
    enter(&mut app, "SrfPtGrid");
    assert!(!app.viewport_drafting_input(false, false).active);
    assert!(!app.accept_drafting_point(point(1., 2., 3.)));
    enter(&mut app, "DegreeU=1 2 DegreeV=1 2");
    assert!(app.viewport_drafting_input(false, false).active);
    app.active_viewport = 1;
    assert!(app.accept_drafting_point(point(1., 2., 3.)));
    enter(&mut app, "r0,3,1");
    app.execute_command("Undo");
    assert_eq!(
        app.surface_point_grid_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap()
            .0,
        &[point(1., 2., 3.)]
    );
    enter(&mut app, ".xy");
    enter(&mut app, "w4,5,6");
    enter(&mut app, "w0,0,9");
    assert_eq!(
        app.surface_point_grid_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap()
            .0
            .last(),
        Some(&point(4., 5., 9.))
    );
    assert!(app.document.objects().next().is_none());
    app.document
        .add_geometry(Geometry::Point(point(8., 9., 10.)))
        .unwrap();
    assert!(!app.accept_drafting_point(point(5., 6., 7.)));
    assert!(app.surface_point_grid_session.is_none());
    enter(&mut app, "SrfControlPtGrid Degree=1 2 Degree=1 2");
    app.document
        .set_tolerance(Tolerance::try_new(0.01, 1e-9, 1e-6).unwrap());
    assert!(!app.accept_drafting_point(point(5., 6., 7.)));
    assert!(app.active_command.is_none());
}

#[test]
fn surface_point_grid_preview_connects_rows_without_inserting_geometry() {
    let mut app = test_app();
    enter(&mut app, "SrfControlPtGrid Degree=1 2 Degree=1 3");
    for p in [
        point(0., 0., 0.),
        point(0., 2., 1.),
        point(0., 5., 0.),
        point(4., 0., 0.),
    ] {
        assert!(app.accept_drafting_point(p));
    }
    let context = egui::Context::default();
    let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
        let (points, count_v) = app
            .surface_point_grid_session
            .as_ref()
            .unwrap()
            .preview()
            .unwrap();
        let points = points.to_vec();
        let input = ViewportInput {
            drafting: app.viewport_drafting_input(false, false),
            surface_point_grid: Some((&points, count_v)),
            ..Default::default()
        };
        let drawn = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                app.viewports[0].show(ui, &app.document, input, &[], 0, true);
            },
        );
        let ink = egui::Color32::from_rgb(20, 115, 190);
        let lines = drawn
            .shapes
            .iter()
            .filter(
                |s| matches!(&s.shape,egui::Shape::LineSegment {stroke,..} if stroke.color==ink),
            )
            .count();
        drawn.drop_without_applying_deltas();
        lines
    };
    frame(&mut app, vec![]);
    assert_eq!(
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(egui::pos2(510., 230.))]
        ),
        5
    );
    assert!(app.document.objects().next().is_none());
    assert!(!app.document.can_undo());
    enter(&mut app, "Cancel");
    assert!(app.surface_point_grid_session.is_none());
}

#[test]
fn invalid_grid_geometry_and_count_undo_preserve_pending_redo() {
    let mut app = test_app();
    app.commands
        .execute(&mut app.document, "Point 1,2,3")
        .unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "SrfPtGrid");
    app.execute_command("Undo");
    assert!(app.document.can_redo());
    assert!(app.surface_point_grid_session.is_some());
    enter(&mut app, "DegreeU=1 2 DegreeV=1 2");
    for _ in 0..3 {
        assert!(app.accept_drafting_point(point(0., 0., 0.)));
    }
    assert!(!app.accept_drafting_point(point(0., 0., 0.)));
    assert!(app.surface_point_grid_session.is_none());
    assert!(app.active_command.is_none());
    assert!(app.document.objects().next().is_none());
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
}
