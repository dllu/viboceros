use super::command_file::replay;
use super::line_reference::inputs as reference_inputs;
use super::plane_surface_native_support as native;
use super::*;
fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
#[test]
fn picked_curve_ends_and_options_match_native_geometry_selection_history_and_archives() {
    let directory = tempfile::tempdir().unwrap();
    let mut count = 0;
    for family in ["arc_blend_command", "arc_blend_options"] {
        for row in native::read(family)["results"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            if ![
                "arc_default",
                "arc_s_shape",
                "arc_spatial",
                "arc_alternate",
                "arc_trim",
                "arc_join",
            ]
            .contains(&id)
            {
                continue;
            }
            let v = &row["value"];
            let source = &v["prescribed_case"];
            let trace = &v["input_trace"][0];
            assert_eq!(trace["completed"], trace["expected"]);
            assert_eq!(trace["errors"], serde_json::json!([]));
            let mut app = test_app();
            app.document = reference_inputs::setup(v, source);
            let parents = app.document.objects().cloned().collect::<Vec<_>>();
            enter(&mut app, "ArcBlend");
            assert_eq!(app.document.selected_objects().count(), 0);
            for action in source["actions"].as_array().unwrap() {
                if let Some(i) = action["curve"].as_u64() {
                    let object = reference_inputs::guide(&app.document, i as usize);
                    assert!(app.pick_arc_blend_at(object, None, native::point(&action["pick"])));
                } else {
                    let word = reference_inputs::token(action);
                    enter(&mut app, if word == "Enter" { "" } else { &word });
                }
                if app.active_command.is_some() {
                    assert!(!app.document.can_undo());
                    for parent in &parents {
                        assert_eq!(app.document.object(parent.id()).unwrap(), parent);
                    }
                }
            }
            assert!(app.active_command.is_none(), "{id}: {:?}", app.command_log);
            let mut error = replay::check(&app.document, &v["after"], false);
            assert_eq!(app.document.undo_label(), Some("ArcBlend"));
            app.document.undo().unwrap();
            error = error.max(replay::check(&app.document, &v["undo"], false));
            assert!(!app.document.can_undo());
            app.document.redo().unwrap();
            error = error.max(replay::check(&app.document, &v["redo"], false));
            let path = directory.path().join("blend.3dm");
            app.commands
                .execute(&mut app.document, &format!("SaveAs {}", path.display()))
                .unwrap();
            let mut restored = Document::new(app.document.tolerance());
            app.commands
                .execute(&mut restored, &format!("Open3dm {}", path.display()))
                .unwrap();
            error = error.max(replay::check(&restored, &v["after"], true));
            assert!(error <= 1e-6, "{id}: {error:e}");
            println!(
                "ARC_BLEND_UI {}",
                serde_json::json!({"id":id,"maximum_coefficient_and_sample_error":error,"archives":1})
            );
            count += 1;
        }
    }
    assert_eq!(count, 6);
}
pub(super) fn scene() -> (VibocerosApp, Vec<ObjectId>) {
    let mut app = test_app();
    for text in ["Line -5,0,0 0,0,0", "Line 5,4,0 5,9,0", "Point -10,-10,0"] {
        app.commands.execute(&mut app.document, text).unwrap();
    }
    let ids = app.document.objects().map(|o| o.id()).collect::<Vec<_>>();
    app.document
        .select_objects_direct([ids[2]], SelectionMode::Replace)
        .unwrap();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    app.smart_track = false;
    (app, ids)
}
pub(super) fn frame(
    app: &mut VibocerosApp,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> (ViewportOutput, egui::Rect) {
    let picking = app.picking_arc_blend() || app.picking_blend_curve();
    let input = ViewportInput {
        drafting: app.viewport_drafting_input(false, false),
        object_filter: picking.then_some(viboceros_command::ObjectSelectionFilter::Curves),
        edge_pick: picking,
        curve_or_edge_pick: picking,
        blend_curve: app
            .blend_curve_session
            .as_ref()
            .filter(|s| !s.flip && app.plane_prompt.is_none())
            .map(|s| s.preview_input()),
        arc_blend: app
            .arc_blend_session
            .as_ref()
            .filter(|_| app.plane_prompt.is_none())
            .map(|s| s.preview_input()),
        edge_curve: app
            .arc_blend_session
            .as_ref()
            .and_then(|s| s.cursor_curve(app.plane_prompt.is_some()))
            .or_else(|| {
                app.blend_curve_session
                    .as_ref()
                    .filter(|_| app.plane_prompt.is_none())?
                    .cursor_curve()
            }),
        preview_curve: app
            .arc_blend_session
            .as_ref()
            .and_then(|s| s.preview_input().preview())
            .or_else(|| app.blend_curve_session.as_ref()?.preview_input().preview()),
        ..Default::default()
    };
    let mut output = ViewportOutput::default();
    let mut rect = egui::Rect::NOTHING;
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
                rect = ui.available_rect_before_wrap();
                output = app.viewports[0].show(ui, &app.document, input, &[], 0, true);
            },
        )
        .drop_without_applying_deltas();
    (output, rect)
}
pub(super) fn click(
    app: &mut VibocerosApp,
    context: &egui::Context,
    p: egui::Pos2,
) -> ViewportOutput {
    frame(app, context, vec![egui::Event::PointerMoved(p)]);
    frame(
        app,
        context,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    frame(
        app,
        context,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
    .0
}
#[test]
fn windowless_mouse_picks_preview_and_enter_commit_one_history_record() {
    let (mut app, ids) = scene();
    let parents = app.document.objects().cloned().collect::<Vec<_>>();
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "ArcBlend");
    assert!(!app.viewport_drafting_input(false, false).active);
    for (i, world) in [point(-0.25, 0., 0.), point(5., 4.25, 0.)]
        .into_iter()
        .enumerate()
    {
        let screen = app.viewports[0].project(world, rect).unwrap();
        let output = click(&mut app, &context, screen);
        assert_eq!(output.selection_click.unwrap().object_id, Some(ids[i]));
        app.handle_viewport_action(output);
        assert_eq!(
            app.arc_blend_session
                .as_ref()
                .unwrap()
                .input
                .reference_count(),
            i + 1
        );
        assert!(!app.document.can_undo());
    }
    let initial = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    enter(&mut app, "Ratio=2");
    let preview = app
        .arc_blend_session
        .as_ref()
        .unwrap()
        .input
        .preview()
        .unwrap()
        .clone();
    assert_ne!(initial, preview);
    for bad in ["Ratio=-1", "RadiusDifference=NaN", "0.25", "Parameter1=999"] {
        enter(&mut app, bad);
        assert_eq!(
            app.arc_blend_session.as_ref().unwrap().input.preview(),
            Some(&preview)
        );
    }
    frame(&mut app, &context, vec![]);
    enter(&mut app, "");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    assert_eq!(
        app.document
            .objects()
            .last()
            .unwrap()
            .geometry()
            .curve_ref()
            .unwrap()
            .to_nurbs()
            .unwrap(),
        preview
    );
    for parent in parents {
        let now = app.document.object(parent.id()).unwrap();
        assert_eq!(now, &parent);
        assert!(
            parent
                .geometry_snapshot()
                .shares_storage_with(now.geometry_snapshot())
        );
    }
    assert_eq!(app.document.undo_label(), Some("ArcBlend"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.can_undo());
    assert_eq!(app.document.selected_objects().count(), 0);
}
#[test]
fn stale_sources_reset_and_cancel_preserves_pending_redo() {
    let (mut app, ids) = scene();
    app.commands
        .execute(&mut app.document, "Point 1,2,3")
        .unwrap();
    app.document.undo().unwrap();
    enter(&mut app, "ArcBlend");
    enter(&mut app, &format!("Curve={} w0,0,0", ids[0]));
    enter(&mut app, &format!("Curve={} w5,4,0", ids[1]));
    assert!(app.arc_blend_session.as_ref().unwrap().input.ready());
    enter(&mut app, "!");
    assert!(app.document.can_redo());
    assert!(!app.document.can_undo());
    enter(&mut app, "ArcBlend");
    app.pick_arc_blend_at(ids[0], None, point(0., 0., 0.));
    app.pick_arc_blend_at(ids[1], None, point(5., 4., 0.));
    app.document.delete_objects([ids[1]]).unwrap();
    app.validate_arc_blend();
    let input = &app.arc_blend_session.as_ref().unwrap().input;
    assert_eq!(input.reference_count(), 0);
    assert!(input.preview().is_none());
    enter(&mut app, "");
    assert!(app.active_command.is_some());
    enter(&mut app, "!");
    assert!(app.active_command.is_none());
}

#[test]
fn command_files_finish_the_blend_and_transparent_cplane_input_keeps_references() {
    let (mut app, ids) = scene();
    enter(&mut app, "ArcBlend");
    app.pick_arc_blend_at(ids[0], None, point(0., 0., 0.));
    enter(&mut app, "CPlane 3Point");
    assert!(app.plane_prompt.is_some());
    assert!(!app.picking_arc_blend());
    assert!(!app.pick_arc_blend_curve(None, None));
    for p in [point(2., 3., 4.), point(3., 3., 4.), point(2., 4., 4.)] {
        assert!(app.handle_viewport_action(ViewportOutput {
            picked_point: Some(p),
            ..Default::default()
        }));
    }
    assert!(app.plane_prompt.is_none());
    assert_eq!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .reference_count(),
        1
    );
    enter(&mut app, "!");
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("arc blend.txt");
    std::fs::write(
        &file,
        format!(
            "_ArcBlend Curve={} w0,0,0 Curve={} w5,4,0 Ratio=2 _Enter _Point w8,9,10",
            ids[0], ids[1]
        ),
    )
    .unwrap();
    enter(&mut app, &format!("ReadCommandFile \"{}\"", file.display()));
    for _ in 0..100 {
        if !app.drain_command_file() {
            break;
        }
    }
    assert_eq!(
        app.last_command_file_result,
        Some(true),
        "{:?}",
        app.command_log
    );
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 5);
    assert_eq!(app.document.undo_label(), Some("ReadCommandFile"));
    app.document.undo().unwrap();
    assert_eq!(app.document.objects().len(), 3);
    assert!(!app.document.can_undo());
}

#[test]
fn windowless_edge_pick_uses_the_surface_boundary_and_retains_its_parent() {
    let mut app = test_app();
    for command in ["Plane 0,0,0 4,4,0", "Line 8,3,0 8,8,0"] {
        app.commands.execute(&mut app.document, command).unwrap();
    }
    let parent = app.document.objects().next().unwrap().clone();
    app.document.clear_history().unwrap();
    app.viewports[0] = Viewport::new(ViewKind::Top);
    app.grid_snap = false;
    app.osnap = false;
    let context = egui::Context::default();
    frame(&mut app, &context, vec![]);
    app.viewports[0].zoom_out(0.5).unwrap();
    let (_, rect) = frame(&mut app, &context, vec![]);
    enter(&mut app, "ArcBlend");
    for (i, world) in [point(3., 0., 0.), point(8., 3.25, 0.)]
        .into_iter()
        .enumerate()
    {
        let screen = app.viewports[0].project(world, rect).unwrap();
        let output = click(&mut app, &context, screen);
        if i == 0 {
            assert!(!output.edge_click.as_ref().unwrap().is_empty());
        }
        app.handle_viewport_action(output);
        assert_eq!(
            app.arc_blend_session
                .as_ref()
                .unwrap()
                .input
                .reference_count(),
            i + 1,
            "{:?}",
            app.command_log
        );
    }
    assert!(
        app.arc_blend_session
            .as_ref()
            .unwrap()
            .input
            .preview()
            .is_some()
    );
    enter(&mut app, "");
    assert!(app.active_command.is_none(), "{:?}", app.command_log);
    let now = app.document.object(parent.id()).unwrap();
    assert_eq!(now, &parent);
    assert!(
        parent
            .geometry_snapshot()
            .shares_storage_with(now.geometry_snapshot())
    );
    assert_eq!(app.document.objects().len(), 3);
    app.document.undo().unwrap();
    assert!(!app.document.can_undo());
}
