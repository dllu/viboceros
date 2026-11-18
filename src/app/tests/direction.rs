//! Command-first selection and ordinary command entry with shared native operands.
use super::direction_support as support;
use super::*;
use serde_json::json;
use viboceros_oracle::brep_direction::{Fixture, snapshot, snapshots};
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

#[test]
fn direction_input_matches_shared_native_geometry_and_history() {
    for (operation, native) in support::cases(false) {
        let f: Fixture = serde_json::from_value(operation.clone()).unwrap();
        let mut app = test_app();
        let tolerance = viboceros_geometry::Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
        app.document = viboceros_document::Document::new(tolerance);
        let source = f.source(tolerance).unwrap();
        let id = app
            .document
            .add_geometry_with_attributes(
                Geometry::Brep(source),
                viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                    .with_name("Direction source"),
            )
            .unwrap();
        app.document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        app.commands
            .execute(&mut app.document, "Group DirectionSource")
            .unwrap();
        if !f.preselect {
            app.document.clear_selection();
        }
        app.document.clear_history().unwrap();
        let before = snapshot(&app.document, id).unwrap();
        enter(&mut app, &f.command().unwrap());
        if !f.preselect {
            assert!(
                app.object_prompt.is_some(),
                "{}: {:?}",
                operation["id"],
                app.command_log
            );
            app.apply_selection_click(SelectionClick {
                object_id: Some(id),
                mode: SelectionMode::Replace,
            });
            enter(&mut app, "");
        }
        assert_eq!(
            app.document.undo_label(),
            Some("Dir"),
            "{}: {:?}",
            operation["id"],
            app.command_log
        );
        let after = snapshot(&app.document, id).unwrap();
        enter(&mut app, "Undo");
        let undo = snapshot(&app.document, id).unwrap();
        enter(&mut app, "Redo");
        let redo = snapshot(&app.document, id).unwrap();
        support::compare(
            &json!({"before":before,"after":after,"undo":undo,"redo":redo}),
            &native,
        );
    }
}

#[test]
fn direction_face_batch_uses_its_typed_scope_and_cancel_keeps_redo() {
    let (operation, native) = support::cases(true)
        .into_iter()
        .find(|(op, _)| op["id"] == "dir-sdk-box-u_reverse-135")
        .unwrap();
    let mut f: Fixture = serde_json::from_value(operation).unwrap();
    f.preselect = false;
    f.sdk = false;
    let mut app = test_app();
    let id = app
        .document
        .add_geometry(Geometry::Brep(f.source(app.document.tolerance()).unwrap()))
        .unwrap();
    app.document.clear_history().unwrap();
    app.commands
        .execute(&mut app.document, "Point 99,99,99")
        .unwrap();
    app.document.undo().unwrap();
    let before = format!("{:?}", app.document);
    let input = f.command().unwrap();
    enter(&mut app, &input);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, &input);
    app.apply_selection_click(SelectionClick {
        object_id: Some(id),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert_eq!(
        app.document.undo_label(),
        Some("Dir"),
        "{:?}",
        app.command_log
    );
    let actual = snapshot(&app.document, id).unwrap();
    // Public face replacement qualifies this explicit batching extension's
    // geometry; its selection history is intentionally a different route.
    support::compare(&actual["geometry"], &native["after"]["geometry"]);
    enter(&mut app, "Undo");
    assert!(!app.document.is_selected(id));
}

#[test]
fn bare_direction_sessions_match_native_provisional_edits_and_endings() {
    for (operation, native) in support::session_cases() {
        let f: Fixture = serde_json::from_value(operation.clone()).unwrap();
        let mut app = test_app();
        let tolerance = Tolerance::try_new(1e-8, 1e-12, 1e-8).unwrap();
        app.document = Document::new(tolerance);
        let id = app
            .document
            .add_geometry_with_attributes(
                Geometry::Brep(f.source(tolerance).unwrap()),
                viboceros_document::ObjectAttributes::on_layer(app.document.current_layer_id())
                    .with_name("Direction source"),
            )
            .unwrap();
        let mut ids = vec![id];
        if f.pair {
            ids.push(
                app.document
                    .add_geometry_with_attributes(
                        Geometry::Brep(f.source(tolerance).unwrap()),
                        viboceros_document::ObjectAttributes::on_layer(
                            app.document.current_layer_id(),
                        )
                        .with_name("Direction source"),
                    )
                    .unwrap(),
            );
        }
        app.document
            .select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
            .unwrap();
        app.commands
            .execute(&mut app.document, "Group DirectionSource")
            .unwrap();
        if !f.preselect {
            app.document.clear_selection();
        }
        app.document.clear_history().unwrap();
        let snap = |app: &VibocerosApp| snapshots(&app.document, &ids, f.pair).unwrap();
        let before = snap(&app);
        let storage = ids
            .iter()
            .map(|id| {
                app.document
                    .object(*id)
                    .unwrap()
                    .geometry_snapshot()
                    .clone()
            })
            .collect::<Vec<_>>();
        enter(&mut app, "_Direction");
        if !f.preselect {
            assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
            for &id in &ids {
                app.apply_selection_click(SelectionClick {
                    object_id: Some(id),
                    mode: SelectionMode::Add,
                });
            }
            enter(&mut app, "");
        }
        assert!(app.direction_session.is_some(), "{:?}", app.command_log);
        assert!(!app.command_line_idle());
        for step in f.sequence.as_ref().unwrap() {
            enter(&mut app, step);
            for (id, storage) in ids.iter().zip(&storage) {
                assert!(
                    storage
                        .shares_storage_with(app.document.object(*id).unwrap().geometry_snapshot())
                );
            }
            assert!(!app.document.can_undo());
        }
        if f.cancel {
            app.cancel_current_prompt_or_selection();
        } else {
            enter(&mut app, "");
        }
        assert!(app.command_line_idle());
        let after = snap(&app);
        let changed = if f.pair {
            before
                .as_array()
                .unwrap()
                .iter()
                .zip(after.as_array().unwrap())
                .any(|(a, b)| a["geometry"] != b["geometry"])
        } else {
            before["geometry"] != after["geometry"]
        };
        assert_eq!(app.document.can_undo(), changed, "{}", operation["id"]);
        if changed {
            enter(&mut app, "Undo");
        }
        let undo = snap(&app);
        if changed {
            enter(&mut app, "Redo");
        }
        let redo = snap(&app);
        support::compare(
            &json!({"before":before,"after":after,"undo":undo,"redo":redo}),
            &native,
        );
    }
}

#[test]
fn direction_clicks_modes_noops_and_cancel_preserve_document_and_redo() {
    use crate::viewport::ViewportOutput;
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 4,2,0");
    let id = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Point 8,8,8");
    enter(&mut app, "Undo");
    let before = format!("{:?}", app.document);
    enter(&mut app, "Dir");
    assert!(!app.handle_viewport_action(ViewportOutput::default()));
    assert!(app.handle_viewport_action(ViewportOutput {
        direction_click: Some(id),
        ..Default::default()
    }));
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Mode=FlipU");
    // A failed edit cannot discard a successful provisional curve reversal.
    enter(&mut app, "FlipAll");
    assert!(app.command_log.back().unwrap().contains("Error:"));
    enter(&mut app, "Mode");
    enter(&mut app, "FlipNormal");
    app.handle_viewport_action(ViewportOutput {
        direction_click: Some(id),
        ..Default::default()
    });
    enter(&mut app, "");
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.can_redo());
    enter(&mut app, "Dir");
    enter(&mut app, "NextMode");
    enter(&mut app, "Flip");
    app.cancel_current_prompt_or_selection();
    assert_eq!(format!("{:?}", app.document), before);
    assert!(app.document.can_redo());
}

#[test]
fn direction_navigation_keeps_preview_and_source_edits_invalidate_it() {
    let mut app = test_app();
    enter(&mut app, "Line -2,0,0 2,0,0");
    let id = app.document.objects().last().unwrap().id();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    enter(&mut app, "Dir");
    enter(&mut app, "Flip");
    enter(&mut app, "'_SetDisplayMode Shaded");
    assert!(app.direction_session.is_some());
    assert_eq!(
        app.viewports[app.active_viewport].display_mode,
        DisplayMode::Shaded
    );
    assert!(!app.document.can_undo());
    app.commands
        .execute(&mut app.document, "Move 0,0,0 0,0,5")
        .unwrap();
    let before = format!("{:?}", app.document);
    app.validate_direction();
    assert!(app.direction_session.is_none());
    assert_eq!(format!("{:?}", app.document), before);
}

fn inspect_in_top_view(app: &mut VibocerosApp) -> egui::Context {
    use super::circle_preview::viewport_frame;
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    viewport_frame(app, &context, Vec::new());
    viewport_frame(app, &context, Vec::new());
    app.viewports[0]
        .zoom_extents(&app.document, Default::default())
        .unwrap();
    viewport_frame(app, &context, Vec::new());
    enter(app, "Dir");
    assert!(app.direction_session.is_some());
    context
}

fn hover_direction(app: &mut VibocerosApp, context: &egui::Context, point: Point3) -> egui::Pos2 {
    use super::circle_preview::{screen_point, viewport_frame};
    // Starting or editing Dir can resize the log panel above the viewport.
    for _ in 0..6 {
        let pixel = screen_point(app, point);
        viewport_frame(app, context, vec![egui::Event::PointerMoved(pixel)]);
        if screen_point(app, point) == pixel {
            return pixel;
        }
    }
    panic!("viewport did not settle at {point:?}");
}

#[test]
fn direction_status_uses_the_analytic_curve_parameter_and_clears_off_the_curve() {
    use super::circle_preview::viewport_click;
    let mut app = test_app();
    app.execute_command("Circle w0,0,0 5");
    app.execute_command("SelAll");
    app.document.clear_history().unwrap();
    let t = 1.75;
    let target = app
        .document
        .objects()
        .next()
        .unwrap()
        .geometry()
        .curve_ref()
        .unwrap()
        .evaluate(t)
        .unwrap();
    let context = inspect_in_top_view(&mut app);
    let before = format!("{:?}", app.document);
    let pixel = hover_direction(&mut app, &context, target);
    let parameter = |app: &VibocerosApp| {
        app.direction_status()
            .unwrap()
            .strip_prefix("Parameter = ")
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!(
        (parameter(&app) - t).abs() < 1e-5,
        "{:?}",
        app.direction_status()
    );

    viewport_click(&mut app, &context, pixel);
    hover_direction(&mut app, &context, target);
    assert!(
        (parameter(&app) + t).abs() < 1e-5,
        "{:?}",
        app.direction_status()
    );
    assert_eq!(format!("{:?}", app.document), before);

    hover_direction(&mut app, &context, point(0., 0., 0.));
    assert_eq!(app.direction_status(), None);
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.direction_status(), None);
    assert!(!app.document.can_undo());
}

#[test]
fn direction_status_uses_the_staged_normal_and_the_hovered_view_cplane() {
    let mut app = test_app();
    let surface = viboceros_geometry::NurbsSurface::try_bilinear([
        point(-4., -3., -4.),
        point(4., -3., -4.),
        point(4., 3., 4.),
        point(-4., 3., 4.),
    ])
    .unwrap();
    let source = app
        .document
        .add_geometry(Geometry::NurbsSurface(surface))
        .unwrap();
    app.document
        .select_objects_direct([source], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    let context = inspect_in_top_view(&mut app);
    let target = point(1., 0., 0.);
    hover_direction(&mut app, &context, target);
    assert_eq!(app.direction_status(), Some("Draft angle = 36.8699°"));

    app.viewports[0]
        .set_construction_plane(viboceros_command::construction_plane::WorldPlane::Front.frame());
    hover_direction(&mut app, &context, target);
    assert_eq!(app.direction_status(), Some("Draft angle = 53.1301°"));

    enter(&mut app, "Flip");
    hover_direction(&mut app, &context, target);
    assert_eq!(app.direction_status(), Some("Draft angle = -53.1301°"));
    assert!(!app.document.can_undo());
    enter(&mut app, "");
    assert_eq!(app.document.undo_label(), Some("Dir"));
    assert_eq!(app.direction_status(), None);
}

#[test]
fn direction_status_is_hidden_when_multiple_objects_are_inspected() {
    let mut app = test_app();
    app.execute_command("Line w-4,0,0 w4,0,0");
    app.execute_command("Line w-4,2,0 w4,2,0");
    app.execute_command("SelAll");
    let context = inspect_in_top_view(&mut app);
    hover_direction(&mut app, &context, point(-1., 0., 0.));
    assert_eq!(app.direction_status(), None);
}
