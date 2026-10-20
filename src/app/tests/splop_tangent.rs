use super::*;
use crate::splop_tangent_test_support as support;
use viboceros_geometry::Vector3;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn splop_tangent_prompts_replay_native_contacts_clouds_selection_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let (doc, sources) = support::guided::setup(&value);
            let id = op["id"].as_str().unwrap();
            let mut app = test_app();
            app.document = doc;
            app.viewports[app.active_viewport].set_construction_plane(support::plane(&op));
            let prefs = app.commands.circle_preferences();
            let before = app.document.objects().cloned().collect::<Vec<_>>();
            assert!(app.try_start_interactive_command(&format!(
                "Splop {}",
                support::guided::options(&op).arguments()
            )));
            for (index, action) in op["actions"].as_array().unwrap().iter().enumerate() {
                if let Some(word) = action.as_str() {
                    enter(&mut app, word);
                } else if let Some(n) = action.as_f64() {
                    enter(&mut app, &n.to_string());
                } else if action.is_array() {
                    enter(
                        &mut app,
                        &format!(
                            "w{}",
                            support::guided::token(support::guided::morph::p(action))
                        ),
                    );
                } else {
                    assert!(
                        app.picking_tangent_curve(),
                        "{id} {index} {:?}",
                        app.command_log
                    );
                    assert!(app.pick_tangent_at(
                        sources.guides[action["curve"].as_u64().unwrap() as usize],
                        action["edge"].as_u64().map(|i| i as usize),
                        support::contact(&value, index)
                    ));
                }
                assert_eq!(
                    app.document.objects().cloned().collect::<Vec<_>>(),
                    before,
                    "{id} reference input mutated geometry"
                );
                assert!(app.document.undo_label().is_none());
                assert!(app.document.is_selected(sources.source));
                assert!(
                    sources
                        .guides
                        .iter()
                        .all(|id| !app.document.is_selected(*id))
                );
            }
            if op["finish"] != "CancelReference" {
                assert!(app.tangent_session.is_none());
                assert!(app.picking_orient_surface(), "{id} {:?}", app.command_log);
                assert!(app.accept_orient_surface(sources.target, None));
                assert!(app.accept_drafting_point(sources.target_point));
                assert!(
                    app.accept_drafting_point(sources.direction),
                    "{id} {:?}",
                    app.command_log
                );
                if op["copy"] == true {
                    enter(
                        &mut app,
                        if op["finish"] == "CancelCopies" {
                            "Cancel"
                        } else {
                            "Enter"
                        },
                    );
                }
            }
            assert!(app.active_command.is_none());
            assert!(app.orient_surface_session.is_none());
            assert!(app.tangent_session.is_none());
            assert_eq!(app.commands.circle_preferences(), prefs);
            let mut gap = support::check(&sources, &app.document, &op, &value, "after_script", id);
            if op["finish"] == "CancelReference" {
                assert!(app.document.undo_label().is_none());
                assert!(app.document.redo_label().is_none());
            } else {
                assert_eq!(app.document.undo_label(), Some("Splop"));
                app.document.undo().unwrap();
                gap = gap.max(support::check(
                    &sources,
                    &app.document,
                    &op,
                    &value,
                    "undo",
                    id,
                ));
                app.document.redo().unwrap();
                gap = gap.max(support::check(
                    &sources,
                    &app.document,
                    &op,
                    &value,
                    "redo",
                    id,
                ));
            }
            println!(
                "SPLOP_TANGENT_UI_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
fn setup() -> (VibocerosApp, support::guided::Sources) {
    let (_, v) = support::rows().remove(0);
    let (doc, sources) = support::guided::setup(&v);
    let mut app = test_app();
    app.document = doc;
    (app, sources)
}
#[test]
fn splop_tangent_failed_input_and_cancel_keep_selection_geometry_and_redo() {
    let (mut app, sources) = setup();
    app.commands
        .execute(&mut app.document, "Move 0,0,0 1,0,0")
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    enter(&mut app, "Tangent");
    assert!(app.pick_tangent_at(
        sources.guides[0],
        None,
        Point3::try_from([3., 0., 0.]).unwrap()
    ));
    let anchor = app.tangent_session.as_ref().unwrap().input.anchor();
    enter(&mut app, "-2");
    enter(&mut app, "0");
    assert_eq!(app.tangent_session.as_ref().unwrap().input.anchor(), anchor);
    enter(&mut app, "_rAdIuS=3mm");
    assert!(app.pick_tangent_at(
        sources.guides[1],
        None,
        Point3::try_from([0., 4., 0.]).unwrap()
    ));
    assert!(app.picking_orient_surface());
    enter(&mut app, "Cancel");
    assert!(app.orient_surface_session.is_none());
    assert!(app.tangent_session.is_none());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert!(app.document.is_selected(sources.source));
    assert_eq!(app.document.redo_label(), Some("Move"));
    assert!(app.document.undo_label().is_none());
}
#[test]
fn splop_tangent_stale_contacts_refresh_history_and_keep_original_sources() {
    for change in 0..5 {
        let (mut app, sources) = setup();
        let source = app
            .document
            .object(sources.source)
            .unwrap()
            .geometry()
            .clone();
        assert!(app.try_start_interactive_command("Splop Copy=No"));
        enter(&mut app, "Tangent");
        assert!(app.pick_tangent_at(
            sources.guides[0],
            None,
            Point3::try_from([3., 0., 0.]).unwrap()
        ));
        match change {
            0 => {
                app.document
                    .set_objects_visibility([sources.guides[0]], false)
                    .unwrap();
            }
            1 => {
                app.document
                    .set_objects_locked([sources.guides[0]], true)
                    .unwrap();
            }
            2 => {
                app.document.delete_objects([sources.guides[0]]).unwrap();
            }
            3 => {
                app.document
                    .replace_object_geometries([(
                        sources.guides[0],
                        Geometry::Point(Point3::try_from([0.; 3]).unwrap()),
                    )])
                    .unwrap();
            }
            _ => {
                app.document
                    .set_tolerance(Tolerance::try_new(2e-6, 1e-12, 1e-9).unwrap());
            }
        }
        let prior = app.document.undo_label().map(str::to_owned);
        app.validate_tangent_sources();
        assert!(
            app.tangent_session
                .as_ref()
                .unwrap()
                .input
                .anchor()
                .is_none()
        );
        assert!(app.picking_tangent_curve());
        for word in ["Point", "w3,0,0", "Point", "w0,4,0", "Enter"] {
            enter(&mut app, word);
        }
        assert!(app.picking_orient_surface(), "{:?}", app.command_log);
        assert!(app.accept_orient_surface(sources.target, None));
        assert!(app.accept_drafting_point(sources.target_point));
        assert!(
            app.accept_drafting_point(sources.direction),
            "{:?}",
            app.command_log
        );
        assert_eq!(app.document.undo_label(), Some("Splop"));
        app.document.undo().unwrap();
        assert_eq!(app.document.undo_label(), prior.as_deref());
        assert_eq!(
            app.document.object(sources.source).unwrap().geometry(),
            &source
        );
    }
}
#[test]
fn splop_tangent_viewport_pointer_routes_contacts_and_previews_three_readonly_sections() {
    for edge in [None, Some(0)] {
        let (mut app, sources) = setup();
        let mut first = sources.guides[0];
        if edge.is_some() {
            let geometry=serde_json::from_value::<viboceros_oracle::CircleAroundFixture>(serde_json::json!({"actions":[],"curves":[{"type":"box_brep","min":[0,0,0],"max":[12,6,2]}]})).unwrap().source_geometries(app.document.tolerance()).unwrap().remove(0);
            first = app.document.add_geometry(geometry).unwrap();
        }
        app.document.clear_history().unwrap();
        app.active_viewport = 0;
        app.viewports[0] = crate::viewport::Viewport::new(crate::viewport::ViewKind::Top);
        let context = egui::Context::default();
        let mut rect = egui::Rect::NOTHING;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    rect = ui.available_rect_before_wrap();
                    app.viewports[0].show(
                        ui,
                        &app.document,
                        crate::viewport::ViewportInput::default(),
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        let before = app.document.objects().cloned().collect::<Vec<_>>();
        assert!(app.try_start_interactive_command("Splop Copy=No"));
        enter(&mut app, "Tangent");
        let point = Point3::try_from([3., 0., 0.]).unwrap();
        let pointer = app.viewports[0].project(point, rect).unwrap();
        let output = if let Some(edge) = edge {
            ViewportOutput {
                selection_pointer: Some(pointer),
                edge_click: Some(vec![crate::viewport::EdgePick {
                    object: first,
                    edge,
                }]),
                ..Default::default()
            }
        } else {
            ViewportOutput {
                selection_pointer: Some(pointer),
                selection_click: Some(SelectionClick {
                    object_id: Some(first),
                    mode: SelectionMode::Replace,
                }),
                ..Default::default()
            }
        };
        assert!(app.handle_viewport_action(output));
        assert!(
            app.tangent_session
                .as_ref()
                .unwrap()
                .input
                .anchor()
                .unwrap()
                .distance_to(point)
                .unwrap()
                < 1e-5
        );
        let point = Point3::try_from([0., 4., 0.]).unwrap();
        let pointer = app.viewports[0].project(point, rect).unwrap();
        assert!(app.handle_viewport_action(ViewportOutput {
            selection_pointer: Some(pointer),
            selection_click: Some(SelectionClick {
                object_id: Some(sources.guides[1]),
                mode: SelectionMode::Replace
            }),
            ..Default::default()
        }));
        let session = app.tangent_session.as_ref().unwrap();
        let circle = session.input.preview_current().unwrap();
        let preview = crate::viewport::TangentPreview {
            sphere: session.sphere(),
            contacts: &session.input,
            output: None,
            arc_output: None,
            arc: None,
        };
        let sections = preview.circle_curves(circle).unwrap();
        assert_eq!(sections.len(), 3);
        for (section, normal) in
            sections
                .iter()
                .zip([circle.normal().unwrap(), circle.x_axis(), circle.y_axis()])
        {
            let domain = section.domain();
            for i in 0..37 {
                let point = section
                    .evaluate(domain.start() + (domain.end() - domain.start()) * i as f64 / 36.)
                    .unwrap();
                assert!(
                    (circle.center().distance_to(point).unwrap() - circle.radius()).abs() < 1e-9
                );
                assert!(
                    circle
                        .center()
                        .vector_to(point)
                        .unwrap()
                        .dot(normal.as_vector())
                        .unwrap()
                        .abs()
                        < 1e-9
                );
            }
        }
        for mode in [
            crate::viewport::DisplayMode::Wireframe,
            crate::viewport::DisplayMode::Shaded,
            crate::viewport::DisplayMode::Ghosted,
        ] {
            app.viewports[0].display_mode = mode;
            context
                .run_ui(egui::RawInput::default(), |ui| {
                    app.viewports[0].show(
                        ui,
                        &app.document,
                        crate::viewport::ViewportInput {
                            tangent: Some(preview),
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                })
                .drop_without_applying_deltas();
        }
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
        assert!(app.document.undo_label().is_none());
        enter(&mut app, "Enter");
        assert!(app.tangent_session.is_none());
        assert!(app.picking_orient_surface());
        assert!(app.accept_orient_surface(sources.target, None));
        assert!(app.accept_drafting_point(sources.target_point));
        assert!(app.accept_drafting_point(sources.direction));
        assert_eq!(app.document.undo_label(), Some("Splop"));
    }
}
#[test]
fn splop_tangent_default_radius_is_independent_of_circle_preferences() {
    let (mut app, sources) = setup();
    app.commands
        .execute(&mut app.document, "Circle 0,0,0 7")
        .unwrap();
    app.document.undo().unwrap();
    let prefs = app.commands.circle_preferences();
    assert_eq!(prefs.radius, 7.);
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    enter(&mut app, "Tangent");
    assert!(app.pick_tangent_at(
        sources.guides[0],
        None,
        Point3::try_from([3., 0., 0.]).unwrap()
    ));
    assert!(app.pick_tangent_at(
        sources.guides[1],
        None,
        Point3::try_from([0., 4., 0.]).unwrap()
    ));
    enter(&mut app, "Radius");
    enter(&mut app, "Enter");
    assert!(app.picking_orient_surface(), "{:?}", app.command_log);
    assert!(app.accept_orient_surface(sources.target, None));
    assert!(
        (app.orient_surface_session
            .as_ref()
            .unwrap()
            .deformation(Vector3::try_from([0., 0., 1.]).unwrap())
            .unwrap()
            .placement
            .reference_length
            - 1.)
            .abs()
            < 1e-10
    );
    assert_eq!(app.commands.circle_preferences(), prefs);
    enter(&mut app, "Cancel");
    assert_eq!(app.document.redo_label(), Some("Circle"));
    assert!(app.document.undo_label().is_none());
}
