use super::boolean_intersection::{enter, pick};
use super::circle_preview::{screen_point, viewport_click, viewport_frame};
use super::*;
use std::time::{Duration, Instant};

fn fixture() -> (VibocerosApp, ObjectId) {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 5,0,0");
    let id = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    (app, id)
}
#[test]
fn preselected_ribbon_edits_options_measures_distance_and_retains_sources() {
    let (mut app, id) = fixture();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Ribbon");
    assert!(app.ribbon_session.is_some(), "{:?}", app.command_log);
    enter(&mut app, "Corner");
    enter(&mut app, "Smooth");
    enter(&mut app, "Ortho");
    assert!(app.ribbon_session.is_some());
    enter(&mut app, "Ortho");
    assert!(app.ribbon_session.is_some());
    enter(&mut app, "Distance");
    enter(&mut app, "0");
    assert!(app.ribbon_session.is_some());
    enter(&mut app, "0,0,0");
    enter(&mut app, "0,3,0");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "w0,5,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(app.commands.ribbon_options().distance, 3.);
    assert_eq!(
        app.commands.ribbon_options().corner,
        viboceros_geometry::RibbonCornerStyle::Smooth
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.commands.ribbon_options().distance, 3.);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
}
#[test]
fn command_first_curve_pick_finishes_without_enter_and_releases_selection_on_replay() {
    let (mut app, id) = fixture();
    enter(&mut app, "Ribbon Distance=.5 OutputLayer=Input");
    assert!(app.object_prompt.is_some(), "{:?}", app.command_log);
    pick(&mut app, id);
    assert!(app.ribbon_session.is_some(), "{:?}", app.command_log);
    assert!(app.ribbon_session.as_ref().unwrap().postselected);
    enter(&mut app, "0,2,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "Redo");
    assert_eq!(app.document.selected_object_count(), 0);
}
#[test]
fn canceled_and_stale_ribbon_drafts_do_not_create_outputs_or_history() {
    let (mut app, id) = fixture();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Ribbon");
    enter(&mut app, "Distance");
    enter(&mut app, "0,0,0");
    enter(&mut app, "0,0,0");
    assert!(app.ribbon_session.is_some());
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert!(app.document.is_selected(id));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "Ribbon");
    app.document
        .set_object_names([(id, Some("changed".to_owned()))])
        .unwrap();
    enter(&mut app, "0,2,0");
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn ribbon_subcurve_uses_the_picked_portion_and_the_current_layer() {
    let (mut app, id) = fixture();
    let result_layer = app.document.add_layer("Results", ColorRgb::BLACK).unwrap();
    app.document.set_current_layer(result_layer).unwrap();
    app.document
        .set_object_names([(id, Some("Source line".into()))])
        .unwrap();
    app.document.clear_history().unwrap();
    let source = app.document.object(id).unwrap().clone();

    enter(&mut app, "Ribbon Distance=1");
    enter(&mut app, "_SubCrv");
    assert!(app.subcurve_prompt.is_some());
    enter(&mut app, "");
    assert!(app.object_prompt.is_some());
    assert!(app.subcurve_prompt.is_none());
    enter(&mut app, "_SubCrv");
    pick(&mut app, id);
    assert!(app.command_hint().contains("pick the subcurve start"));
    enter(&mut app, "w1,0,0");
    enter(&mut app, "w4,0,0");
    assert!(app.subcurve_prompt.is_none(), "{:?}", app.command_log);
    let guide = app
        .ribbon_session
        .as_ref()
        .unwrap()
        .portion_preview(&app.document)
        .unwrap();
    let bounds = guide.control_point_bounds();
    assert!(bounds.min().distance_to(point(1., 0., 0.)).unwrap() < 1e-9);
    assert!(bounds.max().distance_to(point(4., 0., 0.)).unwrap() < 1e-9);
    let context = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    let preview = loop {
        app.update_ribbon_preview(Some(point(3., 2., 0.)), &context);
        let session = app.ribbon_session.as_ref().unwrap();
        if let Some(preview) = session.preview_curves(&app.document) {
            break preview;
        }
        assert!(
            Instant::now() < deadline,
            "temporary ribbon preview did not arrive"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(preview.iter().any(|edge| {
        let bounds = edge.control_point_bounds();
        bounds.min().distance_to(point(1., 1., 0.)).unwrap() < 1e-9
            && bounds.max().distance_to(point(4., 1., 0.)).unwrap() < 1e-9
    }));
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());

    enter(&mut app, "OutputLayer=Input");
    enter(&mut app, "w3,2,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let output = app.document.objects().find(|o| o.id() != id).unwrap();
    let Geometry::Brep(brep) = output.geometry() else {
        panic!("expected the ribbon surface");
    };
    // Rhino 8.32 uses the current layer for a temporary source.
    assert_eq!(output.attributes().layer_id(), result_layer);
    assert_eq!(output.attributes().name(), None);
    assert!(brep.bounds().min().distance_to(point(1., 0., 0.)).unwrap() < 1e-9);
    assert!(brep.bounds().max().distance_to(point(4., 1., 0.)).unwrap() < 1e-9);
    assert!(app.commands.ribbon_options().input_layer);
    assert_eq!(app.document.selected_object_count(), 0);
    assert_eq!(app.document.object(id).unwrap(), &source);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 2);
    assert_eq!(app.document.selected_object_count(), 0);
}

#[test]
fn ribbon_subcurve_can_retry_after_a_canceled_portion_and_measure_length() {
    let (mut app, id) = fixture();
    enter(&mut app, "Ribbon SubCrv");
    assert!(app.subcurve_prompt.is_some(), "{:?}", app.command_log);
    enter(&mut app, &format!("_SelID {id}"));
    enter(&mut app, "w1,0,0");
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert!(!app.document.can_undo());

    enter(&mut app, "Ribbon SubCrv");
    enter(&mut app, &format!("_SelID {id}"));
    enter(&mut app, "w1,0,0");
    enter(&mut app, "2");
    enter(&mut app, "w5,0,0");
    let guide = app
        .ribbon_session
        .as_ref()
        .unwrap()
        .portion_preview(&app.document)
        .unwrap();
    assert!(
        guide
            .control_point_bounds()
            .max()
            .distance_to(point(3., 0., 0.))
            .unwrap()
            < 1e-9
    );
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
}

#[test]
fn ribbon_side_and_through_point_preview_follow_the_mouse_and_click_commit() {
    let (mut app, id) = fixture();
    app.document
        .select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    let context = egui::Context::default();
    enter(&mut app, "Ribbon Distance=1");
    assert!(app.ribbon_session.is_some(), "{:?}", app.command_log);
    viewport_frame(&mut app, &context, vec![]);
    let await_rail = |app: &mut VibocerosApp, cursor_y, rail_y| {
        let target = point(3., cursor_y, 0.);
        let deadline = Instant::now() + Duration::from_secs(5);
        let cursor = loop {
            let cursor = screen_point(app, target);
            let segments = viewport_frame(app, &context, vec![egui::Event::PointerMoved(cursor)]);
            let rail = screen_point(app, point(2.5, rail_y, 0.));
            // The command panel can resize the viewport between two frames.
            if cursor == screen_point(app, target)
                && segments.iter().any(|[a, b]| {
                    (a.y - rail.y).abs() < 0.5
                        && (b.y - rail.y).abs() < 0.5
                        && rail.x >= a.x.min(b.x) - 0.5
                        && rail.x <= a.x.max(b.x) + 0.5
                })
            {
                break cursor;
            }
            assert!(
                Instant::now() < deadline,
                "no preview rail at y={rail_y} ({rail:?}), pointer {cursor:?}, viewport {:?}; \
                 pending curves {:?}, {} visible segments, command log {:?}",
                app.viewports[0].screen_rect(),
                app.ribbon_session
                    .as_ref()
                    .and_then(|s| s.preview_curves(&app.document))
                    .map(|edges| edges
                        .iter()
                        .map(|edge| edge.control_point_bounds())
                        .collect::<Vec<_>>()),
                segments.len(),
                app.command_log
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(app.document.objects().len(), 1);
        assert!(!app.document.can_undo());
        cursor
    };
    await_rail(&mut app, 3., 1.);
    await_rail(&mut app, -3., -1.);
    enter(&mut app, "_ThroughPoint");
    await_rail(&mut app, -3., -3.);
    // The default grid snap resolves this cursor position to y=2.
    let target = await_rail(&mut app, 2.25, 2.);
    viewport_click(&mut app, &context, target);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let result = app
        .document
        .objects()
        .find(|object| object.id() != id)
        .unwrap();
    let Geometry::Brep(brep) = result.geometry() else {
        panic!("expected a ruled ribbon");
    };
    assert!((brep.bounds().max().y() - 2.).abs() < 1e-6);
    assert!((app.commands.ribbon_options().distance - 2.).abs() < 1e-6);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(app.document.is_selected(id));
}
