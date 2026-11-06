use super::plane_surface_native_support as baseline;
use super::*;
#[path = "../../../crates/viboceros-command/tests/support/plane_completion.rs"]
#[allow(dead_code, unused_imports)]
mod native;
use viboceros_command::rectangle_input::EnterAction;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn assert_surface_close(actual: &Geometry, expected: &Geometry) {
    let (Geometry::NurbsSurface(a), Geometry::NurbsSurface(b)) = (actual, expected) else {
        panic!()
    };
    assert_eq!(
        [
            a.degree_u(),
            a.degree_v(),
            a.control_point_count_u(),
            a.control_point_count_v()
        ],
        [
            b.degree_u(),
            b.degree_v(),
            b.control_point_count_u(),
            b.control_point_count_v()
        ]
    );
    for (a, b) in a.control_points().iter().zip(b.control_points()) {
        assert_eq!(a.weight(), b.weight());
        assert!(a.point().distance_to(b.point()).unwrap() < 1e-12);
    }
    for (a, b) in a
        .knots_u()
        .iter()
        .chain(a.knots_v())
        .zip(b.knots_u().iter().chain(b.knots_v()))
    {
        assert!((a - b).abs() < 1e-12);
    }
}
#[test]
fn plane_completion_getters_match_native_context_history_and_archives() {
    let capture = native::read("plane_completion_terminal");
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("completion.3dm");
    let mut completed = 0;
    for row in capture["results"].as_array().unwrap() {
        let v = &row["value"];
        let mut app = test_app();
        app.document = native::setup(v);
        let parents = app.document.objects().cloned().collect::<Vec<_>>();
        app.viewports[0].set_construction_plane(native::frame(v));
        assert!(
            app.commands
                .remember_plane_preferences(native::preferences(v))
        );
        if v["success"] != true {
            app.commands
                .execute(&mut app.document, "Point 100,200,300")
                .unwrap();
            app.document.undo().unwrap();
        }
        let tokens = native::tokens(&app.document, v);
        enter(&mut app, "Plane");
        for word in tokens {
            if word == "Enter"
                && let Some(s) = &app.plane_surface_session
                && !app.plane_surface_setting_pending()
                && s.rectangle.input.enter_action() == EnterAction::Cursor
            {
                let plane = s.rectangle.input.plane_surface_mouse_plane().unwrap();
                assert_eq!(app.drafting_plane, Some(plane));
                let cursor = native::pointer(v).on_plane(plane).unwrap().unwrap();
                app.handle_viewport_action(ViewportOutput {
                    drafting_hover: Some(cursor),
                    ..Default::default()
                });
            }
            enter(&mut app, if word == "Enter" { "" } else { &word });
        }
        assert!(
            app.plane_surface_session.is_none(),
            "{} {:?}",
            row["id"],
            app.command_log
        );
        assert!(app.active_command.is_none());
        assert!(app.drafting_plane.is_none());
        assert_eq!(app.viewports[0].construction_plane(), native::frame(v));
        let mut error = native::check(&app.document, v, "after");
        let mut archive_objects = None;
        if v["success"] == true {
            assert_eq!(app.document.undo_label(), Some("Plane"));
            app.document.undo().unwrap();
            error = error.max(native::check(&app.document, v, "undo"));
            assert!(app.document.undo().unwrap().is_none());
            app.document.redo().unwrap();
            error = error.max(native::check(&app.document, v, "redo"));
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(native::check_archive(&restored, v));
            archive_objects = Some(restored.objects().len());
        } else {
            assert!(!app.document.can_undo());
            assert!(app.document.can_redo());
        }
        for parent in parents {
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
            "PLANE_COMPLETION_UI {}",
            serde_json::json!({"id":row["id"],"maximum_coefficient_and_sample_error":error,"archive_objects":archive_objects})
        );
        assert!(error <= 1e-6, "{} {error:e}", row["id"]);
        completed += 1;
    }
    assert_eq!(completed, 53);
}

#[test]
fn plane_direction_enter_waits_for_a_current_cursor_and_commits_the_preview() {
    let mut app = test_app();
    for word in ["Plane 3Point", "w1,2,3", "w4,6,8", "Enter"] {
        enter(&mut app, word);
    }
    enter(&mut app, "Enter");
    assert!(app.plane_surface_session.is_some());
    assert!(app.document.objects().next().is_none());
    let cursor = point(7., -1., 3.);
    let preview = app
        .plane_surface_session
        .as_ref()
        .unwrap()
        .rectangle
        .input
        .preview(cursor)
        .unwrap();
    app.handle_viewport_action(ViewportOutput {
        drafting_hover: Some(cursor),
        ..Default::default()
    });
    enter(&mut app, "Enter");
    assert!(app.plane_surface_session.is_none());
    let Geometry::NurbsSurface(actual) = app.document.objects().next().unwrap().geometry() else {
        panic!()
    };
    let Geometry::NurbsSurface(expected) = viboceros_command::plane_surface::geometry(
        preview,
        viboceros_command::plane_surface::Options::new(Default::default()),
    )
    .unwrap() else {
        panic!()
    };
    assert_surface_close(
        &Geometry::NurbsSurface(actual.clone()),
        &Geometry::NurbsSurface(expected),
    );
}

#[test]
fn plane_stationary_viewport_pointer_survives_getter_transitions_and_script_execution() {
    let mut app = test_app();
    let context = egui::Context::default();
    let pointer = egui::pos2(510., 210.);
    let frame = |app: &mut VibocerosApp, events: Vec<egui::Event>| {
        let mut output = ViewportOutput::default();
        let input = ViewportInput {
            drafting: DraftingInput {
                active: app.plane_surface_session.is_some(),
                ..Default::default()
            },
            angle_plane: app.rectangle_mouse_plane(),
            rectangle_input: app
                .plane_surface_session
                .as_ref()
                .map(|s| s.rectangle.input),
            ..Default::default()
        };
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
                },
            )
            .drop_without_applying_deltas();
        output
    };
    frame(&mut app, vec![]);
    frame(&mut app, vec![egui::Event::PointerMoved(pointer)]);
    for word in ["Plane 3Point", "w1,2,3", "w4,6,8", "Enter"] {
        enter(&mut app, word);
    }
    let input = app.plane_surface_session.as_ref().unwrap().rectangle.input;
    let plane = input.plane_surface_mouse_plane().unwrap();
    assert_eq!(app.rectangle_mouse_plane(), Some(plane));
    let output = frame(&mut app, vec![]);
    let hovered = output.drafting_hover.unwrap();
    assert!(plane.coordinates_of(hovered).unwrap()[2].abs() < 1e-10);
    let cached = app
        .plane_surface_session
        .as_ref()
        .unwrap()
        .cursor
        .unwrap()
        .1;
    assert!(cached.distance_to(hovered).unwrap() < 1e-10);
    let expected = viboceros_command::plane_surface::geometry(
        input.preview(hovered).unwrap(),
        viboceros_command::plane_surface::Options::new(Default::default()),
    )
    .unwrap();
    // Enter before consuming another viewport output uses the same stationary ray.
    enter(&mut app, "Enter");
    assert_surface_close(app.document.objects().next().unwrap().geometry(), &expected);
    app.document.undo().unwrap();
    enter(&mut app, "Plane 3Point w1,2,3 w4,6,8 Enter Enter");
    assert_surface_close(app.document.objects().next().unwrap().geometry(), &expected);
}
