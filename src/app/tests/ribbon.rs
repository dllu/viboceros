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
fn ribbon_source_options_carry_through_the_pick_and_escape() {
    use viboceros_geometry::RibbonCornerStyle;

    let (mut app, id) = fixture();
    enter(&mut app, "Ribbon");
    for option in [
        "Distance 2",
        "Corner",
        "Round",
        "InCPlane=Yes",
        "OutputLayer",
        "Input",
        "_Tolerance",
        "0.01",
        "_Trim=_No",
        "_Loose",
    ] {
        enter(&mut app, option);
        assert!(
            app.object_prompt.is_some(),
            "{option}: {:?}",
            app.command_log
        );
    }
    enter(&mut app, "Corner=Chamfer");
    app.cancel_current_prompt_or_selection();
    let options = app.commands.ribbon_options();
    assert_eq!(options.distance, 2.);
    assert_eq!(options.corner, RibbonCornerStyle::Round);
    assert_eq!(options.tolerance, 0.01);
    assert!(options.loose && options.in_cplane && options.input_layer && !options.trim);
    assert!(app.command_line_idle());
    assert!(!app.document.can_undo());

    enter(&mut app, "Ribbon");
    for option in ["Loose=No", "BothSides", "ThroughPoint", "Tolerance 0.02"] {
        enter(&mut app, option);
    }
    pick(&mut app, id);
    assert!(
        app.command_hint().contains("Through point"),
        "{:?}",
        app.command_log
    );
    enter(&mut app, "w3,4,0");
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|obj| obj.id() != id)
        .unwrap()
        .geometry()
    else {
        panic!("expected a ruled ribbon");
    };
    assert_eq!(result.faces().len(), 2);
    assert_eq!(result.bounds().min(), point(0., -4., 0.));
    assert_eq!(result.bounds().max(), point(5., 4., 0.));
    assert_eq!(app.commands.ribbon_options().distance, 4.);
    assert!(!app.commands.ribbon_options().both_sides);
    assert!(app.command_line_idle());

    enter(&mut app, "Ribbon");
    pick(&mut app, id);
    enter(&mut app, "Distance=2.5");
    app.cancel_current_prompt_or_selection();
    assert_eq!(app.commands.ribbon_options().distance, 2.5);
    assert_eq!(app.document.objects().len(), 2);
}

#[test]
fn ribbon_source_distance_accepts_a_viewport_pick_and_a_constrained_point() {
    let (mut app, id) = fixture();
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Ribbon");
    enter(&mut app, "Distance");
    viewport_frame(&mut app, &context, vec![]);
    let start = screen_point(&app, point(0., 0., 0.));
    viewport_click(&mut app, &context, start);
    assert!(
        app.command_hint().contains("second distance point"),
        "{:?}",
        app.command_log
    );
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "3");
    assert!(app.command_hint().contains("second distance point"));
    enter(&mut app, "w3,4,0");
    assert!(
        app.command_hint().contains("Select a curve"),
        "{:?}",
        app.command_log
    );
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    pick(&mut app, id);
    enter(&mut app, "w3,4,0");
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|obj| obj.id() != id)
        .unwrap()
        .geometry()
    else {
        panic!("expected a ruled ribbon");
    };
    assert!((result.bounds().max().y() - 3.).abs() < 1e-9);
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
fn ribbon_trim_edits_before_and_after_source_pick_update_the_preview() {
    let mut app = test_app();
    enter(&mut app, "Polyline 0,0,0 6,0,0 6,4,0");
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Ribbon");
    enter(&mut app, "_Trim=_No");
    pick(&mut app, source);
    assert!(app.ribbon_session.is_some(), "{:?}", app.command_log);
    let context = egui::Context::default();
    let has_full_inner_strip = |app: &mut VibocerosApp| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update_ribbon_preview(Some(point(3., 2., 0.)), &context);
            if let Some(preview) = app
                .ribbon_session
                .as_ref()
                .unwrap()
                .preview_curves(&app.document)
            {
                return preview.iter().any(|edge| {
                    let bounds = edge.control_point_bounds();
                    bounds.min().distance_to(point(0., 1., 0.)).unwrap() < 1e-9
                        && bounds.max().distance_to(point(6., 1., 0.)).unwrap() < 1e-9
                });
            }
            assert!(Instant::now() < deadline, "Ribbon preview did not arrive");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert!(has_full_inner_strip(&mut app));
    enter(&mut app, "_Trim");
    assert!(!has_full_inner_strip(&mut app));
    enter(&mut app, "Trim No");
    assert!(has_full_inner_strip(&mut app));
    assert!(!app.document.can_undo());
    enter(&mut app, "w3,2,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(ribbon) = app
        .document
        .objects()
        .find(|o| o.id() != source)
        .unwrap()
        .geometry()
    else {
        panic!("expected a ribbon");
    };
    assert_eq!((ribbon.faces().len(), ribbon.edges().len()), (2, 8));
    assert!(!app.commands.ribbon_options().trim);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.commands.ribbon_options().trim);
    enter(&mut app, "Ribbon");
    enter(&mut app, "Trim=Yes");
    app.cancel_current_prompt_or_selection();
    assert!(app.command_line_idle());
    assert!(app.commands.ribbon_options().trim);
    assert!(!app.document.can_undo());
}

#[test]
fn ribbon_trim_preview_splits_an_inward_offset_at_a_narrow_neck() {
    let mut app = test_app();
    let context = egui::Context::default();
    enter(
        &mut app,
        "Polyline 0,0,0 4,0,0 4,1.5,0 8,1.5,0 8,0,0 12,0,0 12,4,0 8,4,0 8,2.5,0 4,2.5,0 4,4,0 0,4,0 0,0,0",
    );
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Ribbon .75 Trim=No");
    pick(&mut app, source);
    let target = point(2., 2., 0.);
    let edge_count = |app: &mut VibocerosApp| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update_ribbon_preview(Some(target), &context);
            if let Some(edges) = app
                .ribbon_session
                .as_ref()
                .unwrap()
                .preview_curves(&app.document)
            {
                return edges.len();
            }
            assert!(Instant::now() < deadline, "Ribbon preview did not complete");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert_eq!(edge_count(&mut app), 44);
    enter(&mut app, "Trim=Yes");
    assert_eq!(edge_count(&mut app), 32);
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    enter(&mut app, "w2,2,0");
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|object| object.id() != source)
        .unwrap()
        .geometry()
    else {
        panic!("expected a ruled ribbon");
    };
    assert_eq!((result.faces().len(), result.edges().len()), (10, 32));
}

fn await_ribbon_preview(
    app: &mut VibocerosApp,
    context: &egui::Context,
    target: Point3,
    count: usize,
) -> egui::Pos2 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let cursor = screen_point(app, target);
        viewport_frame(app, context, vec![egui::Event::PointerMoved(cursor)]);
        if cursor == screen_point(app, target)
            && app
                .ribbon_session
                .as_ref()
                .and_then(|session| session.preview_curves(&app.document))
                .is_some_and(|edges| edges.len() == count)
        {
            return cursor;
        }
        assert!(
            Instant::now() < deadline,
            "Ribbon did not preview {count} edges at {target:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn ribbon_coincident_open_rails_follow_the_side_pick_at_fixed_width() {
    let mut app = test_app();
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Polyline 0,0,0 6,0,0 6,2,0 0,2,0");
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Ribbon 1 Trim=Yes");
    pick(&mut app, source);
    viewport_frame(&mut app, &context, vec![]);
    await_ribbon_preview(&mut app, &context, point(2., 1., 0.), 13);
    let cursor = await_ribbon_preview(&mut app, &context, point(5., 1., 0.), 22);
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, cursor);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let topology = |app: &VibocerosApp| {
        let Geometry::Brep(brep) = app
            .document
            .objects()
            .find(|object| object.id() != source)
            .unwrap()
            .geometry()
        else {
            panic!("expected a ruled ribbon");
        };
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len(),
        )
    };
    assert_eq!(topology(&app), (6, 22, 15));
    enter(&mut app, "Undo");
    enter(&mut app, "Ribbon");
    pick(&mut app, source);
    enter(&mut app, "w2,1,0");
    assert_eq!(topology(&app), (5, 13, 8));
}

#[test]
fn ribbon_both_sides_previews_the_picked_walk_at_a_short_corner() {
    let mut app = test_app();
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Polyline 5,0,0 6,0,0 6,4,0");
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Ribbon 1.25 BothSides=Yes Trim=Yes");
    pick(&mut app, source);
    viewport_frame(&mut app, &context, vec![]);
    await_ribbon_preview(&mut app, &context, point(5.5, 1., 0.), 15);
    let outside = await_ribbon_preview(&mut app, &context, point(5.5, -1., 0.), 11);
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, outside);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(brep) = app
        .document
        .objects()
        .find(|object| object.id() != source)
        .unwrap()
        .geometry()
    else {
        panic!("expected a ruled ribbon");
    };
    assert_eq!((brep.faces().len(), brep.edges().len()), (3, 11));
}

#[test]
fn ribbon_both_sides_joins_coincident_open_end_offsets() {
    let mut app = test_app();
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Polyline 0,0,0 6,0,0 6,2,0 0,2,0");
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Ribbon 1 Trim=Yes");
    pick(&mut app, source);
    viewport_frame(&mut app, &context, vec![]);
    let mouth = point(2., 1., 0.);
    await_ribbon_preview(&mut app, &context, mouth, 13);
    enter(&mut app, "BothSides=Yes");
    await_ribbon_preview(&mut app, &context, mouth, 19);
    let outside = await_ribbon_preview(&mut app, &context, point(5., -1., 0.), 19);
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, outside);
    let topology = |app: &VibocerosApp| {
        assert!(app.command_line_idle(), "{:?}", app.command_log);
        let Geometry::Brep(brep) = app
            .document
            .objects()
            .find(|object| object.id() != source)
            .unwrap()
            .geometry()
        else {
            panic!("expected a ruled ribbon");
        };
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len(),
        )
    };
    assert_eq!(topology(&app), (8, 19, 12));
    enter(&mut app, "Undo");
    enter(&mut app, "Ribbon BothSides=Yes ThroughPoint");
    pick(&mut app, source);
    enter(&mut app, "w5,-1,0");
    assert_eq!(topology(&app), (8, 19, 12));
}

#[test]
fn ribbon_corner_hover_and_click_use_the_same_side() {
    let mut app = test_app();
    let context = egui::Context::default();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.maximized_viewport = Some(0);
    enter(&mut app, "Polyline 0,0,0 6,0,0 2,4,0");
    let source = app.document.objects().next().unwrap().id();
    app.document.clear_selection();
    app.document.clear_history().unwrap();
    enter(&mut app, "Ribbon .25 Trim=No");
    pick(&mut app, source);
    viewport_frame(&mut app, &context, vec![]);

    let target = point(7., -2., 0.);
    let deadline = Instant::now() + Duration::from_secs(5);
    let cursor = loop {
        let cursor = screen_point(&app, target);
        viewport_frame(&mut app, &context, vec![egui::Event::PointerMoved(cursor)]);
        let has_rail = app
            .ribbon_session
            .as_ref()
            .unwrap()
            .preview_curves(&app.document)
            .is_some_and(|edges| {
                edges.iter().any(|edge| {
                    let bounds = edge.control_point_bounds();
                    (bounds.min().y() + 0.25).abs() < 1e-9 && (bounds.max().y() + 0.25).abs() < 1e-9
                })
            });
        if has_rail && cursor == screen_point(&app, target) {
            break cursor;
        }
        assert!(
            Instant::now() < deadline,
            "Ribbon preview did not show the outside rail"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(app.document.objects().len(), 1);
    assert!(!app.document.can_undo());
    viewport_click(&mut app, &context, cursor);
    assert!(app.command_line_idle(), "{:?}", app.command_log);
    let Geometry::Brep(result) = app
        .document
        .objects()
        .find(|obj| obj.id() != source)
        .unwrap()
        .geometry()
    else {
        panic!("expected a ruled ribbon");
    };
    assert!((result.bounds().min().y() + 0.25).abs() < 1e-9);
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
