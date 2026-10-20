use super::*;
use crate::splop_guided_test_support as support;
fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}
#[test]
fn splop_guided_reference_prompts_replay_native_clouds_selection_and_history() {
    let mut failures = Vec::new();
    for (op, value) in support::rows() {
        let result = std::panic::catch_unwind(|| {
            let (doc, sources) = support::setup(&value);
            let id = op["id"].as_str().unwrap();
            let mut app = test_app();
            app.document = doc;
            app.viewports[app.active_viewport].set_construction_plane(support::plane(&op));
            sources.check(&app.document, &value, "before", id);
            assert!(app.try_start_interactive_command(&format!(
                "Splop {}",
                support::options(&op).arguments()
            )));
            if op["mode"] == "Fit" {
                enter(&mut app, "FitPoints");
                assert!(app.picking_splop_fit_points());
                assert!(app.control_point_picking_available());
                if op["source"] == "Points" {
                    for guide in &sources.guides {
                        enter(&mut app, &format!("SelID {guide}"));
                    }
                } else {
                    enter(&mut app, "SelPt");
                }
                enter(&mut app, "Enter");
                assert!(app.object_prompt.is_none(), "{id} {:?}", app.command_log);
            } else if op["mode"] == "Around" {
                enter(&mut app, "AroundCurve");
                assert!(app.picking_splop_reference_curve());
                app.apply_selection_click(SelectionClick {
                    object_id: Some(sources.guides[0]),
                    mode: SelectionMode::Replace,
                });
                assert!(
                    app.accept_drafting_point(support::morph::p(&value["reference"]["center"]))
                );
                if op["size"] == "Point" {
                    assert!(app.accept_drafting_point(support::morph::p(
                        &value["reference"]["size_point"]
                    )));
                } else {
                    let (name, size) = support::size(&op);
                    enter(&mut app, name);
                    enter(&mut app, &size.to_string());
                }
            } else {
                for word in support::tokens(&value) {
                    enter(&mut app, word);
                }
            }
            assert!(app.picking_orient_surface(), "{id} {:?}", app.command_log);
            assert!(app.document.undo_label().is_none());
            if op["finish"] == "CancelReference" {
                app.cancel_interactive_command(true);
            } else {
                assert!(app.accept_orient_surface(sources.target, None));
                assert!(app.accept_drafting_point(sources.target_point));
                assert!(app.accept_drafting_point(sources.direction));
                if app.active_command.is_some() {
                    enter(&mut app, "Enter");
                }
            }
            assert!(app.active_command.is_none());
            assert!(app.orient_surface_session.is_none());
            let mut gap = sources.check(&app.document, &value, "after_script", id);
            if op["finish"] == "CancelReference" {
                assert!(app.document.undo_label().is_none());
                assert!(app.document.redo_label().is_none());
            } else {
                assert_eq!(app.document.undo_label(), Some("Splop"));
                app.document.undo().unwrap();
                gap = gap.max(sources.check(&app.document, &value, "undo", id));
                app.document.redo().unwrap();
                gap = gap.max(sources.check(&app.document, &value, "redo", id));
            }
            println!(
                "SPLOP_GUIDED_UI_AUDIT {}",
                serde_json::json!({"id":op["id"],"maximum_error":gap})
            );
        });
        if result.is_err() {
            failures.push(op["id"].clone());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
#[test]
fn splop_fit_reference_retry_and_cancel_preserve_redo_and_all_geometry() {
    let (_, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["id"] == "guided_0")
        .unwrap();
    let (doc, sources) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    app.commands
        .execute(&mut app.document, "Move 0,0,0 1,0,0")
        .unwrap();
    app.document.undo().unwrap();
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    enter(&mut app, "FitPoints");
    for id in &sources.guides[..2] {
        app.apply_selection_click(SelectionClick {
            object_id: Some(*id),
            mode: SelectionMode::Replace,
        });
    }
    // Point fitting accumulates picks through the ordinary GetObject controller.
    enter(&mut app, "");
    assert!(app.picking_splop_fit_points());
    assert_eq!(app.document.redo_label(), Some("Move"));
    for id in &sources.guides {
        enter(&mut app, &format!("SelID {id}"));
    }
    enter(&mut app, "");
    assert!(app.picking_orient_surface());
    assert!(app.object_prompt.is_none());
    app.cancel_interactive_command(true);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(app.document.redo_label(), Some("Move"));
    assert!(app.document.undo_label().is_none());
    assert_eq!(app.document.selected_object_count(), 0);
}
#[test]
fn splop_around_stale_guides_reset_picking_and_surface_edges_use_component_routing() {
    let (_, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["mode"] == "Around" && op["kind"] == "LineZ")
        .unwrap();
    let (doc, sources) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    enter(&mut app, "AroundCurve");
    assert!(app.pick_splop_reference_curve(Some(sources.guides[0]), None));
    app.document
        .set_objects_visibility([sources.guides[0]], false)
        .unwrap();
    let prior_undo = app.document.undo_label().map(str::to_owned);
    assert!(!app.accept_drafting_point(support::morph::p(&value["reference"]["center"])));
    assert!(app.picking_splop_reference_curve());
    assert_eq!(app.document.undo_label(), prior_undo.as_deref());
    let before = app
        .document
        .object(sources.target)
        .unwrap()
        .geometry()
        .clone();
    assert!(app.handle_viewport_action(crate::viewport::ViewportOutput {
        edge_click: Some(vec![crate::viewport::EdgePick {
            object: sources.target,
            edge: 0
        }]),
        ..Default::default()
    }));
    assert!(!app.picking_splop_reference_curve());
    let edge = viboceros_command::curve_reference::CapturedCurve::capture(
        &app.document,
        sources.target,
        Some(0),
    )
    .unwrap();
    let domain = edge.curve().domain();
    let point = edge
        .curve()
        .evaluate((domain.start() + domain.end()) * 0.5)
        .unwrap();
    assert!(app.accept_drafting_point(point));
    enter(&mut app, "2");
    assert!(app.picking_orient_surface());
    assert!(app.accept_orient_surface(sources.target, None));
    assert!(app.accept_drafting_point(sources.target_point));
    assert!(app.accept_drafting_point(sources.direction));
    assert_eq!(app.document.undo_label(), Some("Splop"));
    assert_eq!(
        app.document.object(sources.target).unwrap().geometry(),
        &before
    );
}
#[test]
fn splop_four_point_radius_rejects_small_sizes_and_missing_center_side() {
    let (_, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["mode"] == "FourRadius")
        .unwrap();
    let (doc, _) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    for word in [
        "4Point", "w2,0,0", "w0,2,0", "w-2,0,0", "Radius", "1", "Enter",
    ] {
        enter(&mut app, word);
    }
    assert!(!app.picking_orient_surface());
    assert!(app.document.undo_label().is_none());
    enter(&mut app, "3mm");
    assert!(!app.accept_drafting_point(Point3::try_from([0.; 3]).unwrap()));
    assert!(app.accept_drafting_point(Point3::try_from([0., 0., 4.]).unwrap()));
    assert!(app.picking_orient_surface());
    assert!(app.document.undo_label().is_none());
    app.cancel_interactive_command(true);
}
#[test]
fn splop_around_area_rejects_length_units_and_keeps_circle_preferences() {
    let (_, value) = support::rows()
        .into_iter()
        .find(|(op, _)| op["mode"] == "Around" && op["kind"] == "LineZ")
        .unwrap();
    let (doc, sources) = support::setup(&value);
    let mut app = test_app();
    app.document = doc;
    let preferences = app.commands.circle_preferences();
    assert!(app.try_start_interactive_command("Splop Copy=No"));
    enter(&mut app, "AroundCurve");
    enter(&mut app, &format!("Curve={}", sources.guides[0]));
    assert!(app.accept_drafting_point(support::morph::p(&value["reference"]["center"])));
    enter(&mut app, "Area");
    enter(&mut app, "4mm");
    assert!(!app.picking_orient_surface());
    assert!(app.point_constraint.is_none());
    assert!(app.document.undo_label().is_none());
    enter(&mut app, &(4. * std::f64::consts::PI).to_string());
    assert!(app.picking_orient_surface());
    assert_eq!(app.commands.circle_preferences(), preferences);
    app.cancel_interactive_command(true);
}
