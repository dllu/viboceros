use super::*;

#[test]
fn zoom_window_drag_preserves_an_unfinished_modeling_command() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "Point 4,5,6");
    enter(&mut app, "Undo");
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "Zoom Window");
    assert!(app.zoom_window_pending);
    assert_eq!(app.active_command, pending);

    let context = egui::Context::default();
    let pointer_event = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    zoom_window_frame(&context, &mut app, vec![]);
    let start = egui::Pos2::new(200.0, 150.0);
    let end = egui::Pos2::new(400.0, 350.0);
    zoom_window_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(start), pointer_event(start, true)],
    );
    zoom_window_frame(&context, &mut app, vec![egui::Event::PointerMoved(end)]);
    let output = zoom_window_frame(&context, &mut app, vec![pointer_event(end, false)]);
    assert_eq!(output.zoom_window_result, Some(Ok(true)));
    assert!(output.selection_window.is_none());
    assert!(output.selection_click.is_none());
    assert!(output.picked_point.is_none());
    assert!(app.handle_viewport_action(output));
    assert!(!app.zoom_window_pending);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "Zoom");
    assert!(app.zoom_window_pending);
    let right_event = |pressed| egui::Event::PointerButton {
        pos: end,
        pressed,
        button: egui::PointerButton::Secondary,
        modifiers: egui::Modifiers::NONE,
    };
    zoom_window_frame(&context, &mut app, vec![right_event(true)]);
    let output = zoom_window_frame(&context, &mut app, vec![right_event(false)]);
    assert!(output.zoom_window_cancelled);
    assert!(!output.enter_pressed);
    assert!(app.handle_viewport_action(output));
    assert!(!app.zoom_window_pending);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "Zoom");
    assert!(app.zoom_window_pending);
    enter(&mut app, "ZE");
    assert!(!app.zoom_window_pending);
    assert_eq!(app.active_command, pending);
}

#[test]
fn zoom_target_two_clicks_preserve_the_modeling_prompt_and_redo() {
    let mut app = test_app();
    for command in ["Point 2,3,4", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    let context = egui::Context::default();
    enter(&mut app, "ZT");
    zoom_target_frame(&context, &mut app, ZoomTargetInput::PickTarget, vec![]);
    let initial = app.viewports[0].camera_snapshot();
    let click = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    let center = egui::Pos2::new(400.0, 300.0);
    zoom_target_frame(
        &context,
        &mut app,
        ZoomTargetInput::PickTarget,
        vec![egui::Event::PointerMoved(center), click(center, true)],
    );
    let output = zoom_target_frame(
        &context,
        &mut app,
        ZoomTargetInput::PickTarget,
        vec![click(center, false)],
    );
    let (target, viewport) = output.zoom_target_pick.unwrap();
    assert_eq!(viewport, 0);
    assert!(output.picked_point.is_none() && output.selection_click.is_none());
    assert!(app.handle_viewport_action(output));
    assert!(matches!(
        app.zoom_target,
        Some(ZoomTargetState::PickWindow { .. })
    ));
    let corner = egui::Pos2::new(500.0, 375.0);
    zoom_target_frame(
        &context,
        &mut app,
        ZoomTargetInput::PickWindow(target),
        vec![egui::Event::PointerMoved(corner), click(corner, true)],
    );
    let output = zoom_target_frame(
        &context,
        &mut app,
        ZoomTargetInput::PickWindow(target),
        vec![click(corner, false)],
    );
    assert_eq!(output.zoom_target_result, Some(Ok(true)));
    assert!(output.picked_point.is_none() && output.selection_click.is_none());
    assert!(app.handle_viewport_action(output));
    assert!(app.zoom_target.is_none());
    assert_ne!(app.viewports[0].camera_snapshot(), initial);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[0].camera_snapshot(), initial);

    enter(&mut app, "ZT");
    let right = |pressed| egui::Event::PointerButton {
        pos: center,
        pressed,
        button: egui::PointerButton::Secondary,
        modifiers: egui::Modifiers::NONE,
    };
    zoom_target_frame(
        &context,
        &mut app,
        ZoomTargetInput::PickTarget,
        vec![right(true)],
    );
    let output = zoom_target_frame(
        &context,
        &mut app,
        ZoomTargetInput::PickTarget,
        vec![right(false)],
    );
    assert!(output.zoom_target_cancelled);
    assert!(!output.enter_pressed);
    assert!(app.handle_viewport_action(output));
    assert!(app.zoom_target.is_none());
    assert_eq!(app.active_command, pending);
}

#[test]
fn zoom_target_accepts_typed_points_without_consuming_model_input() {
    let mut app = test_app();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    for command in ["Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    enter(&mut app, "Zoom Target");
    enter(&mut app, "w2,1,0");
    assert!(matches!(
        app.zoom_target,
        Some(ZoomTargetState::PickWindow { .. })
    ));
    enter(&mut app, "w2,1,0");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert!(app.zoom_target.is_some());
    enter(&mut app, "w3,2,0");
    assert!(app.zoom_target.is_none());
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().len(), 0);
    assert!(app.viewports[0].undo_view());
}

#[test]
fn view_history_is_per_viewport_and_independent_of_model_history() {
    let mut app = test_app();
    for command in ["Point 1,2,3", "Point 4,5,6", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let original = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "UndoView");
    assert_eq!(
        app.command_log.back().unwrap(),
        "No viewport change to undo"
    );
    enter(&mut app, "Zoom Factor 2");
    let changed_top = app.viewports[0].camera_snapshot();
    assert_ne!(changed_top, original[0]);
    app.active_viewport = 1;
    enter(&mut app, "Zoom Factor 3");
    let changed_perspective = app.viewports[1].camera_snapshot();
    assert_ne!(changed_perspective, original[1]);
    enter(&mut app, "'_UndoView");
    assert_eq!(app.viewports[1].camera_snapshot(), original[1]);
    assert_eq!(app.viewports[0].camera_snapshot(), changed_top);
    enter(&mut app, "RedoView");
    assert_eq!(app.viewports[1].camera_snapshot(), changed_perspective);
    app.active_viewport = 0;
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[0].camera_snapshot(), original[0]);
    assert_eq!(app.viewports[1].camera_snapshot(), changed_perspective);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.redo_label(), redo.as_deref());
}

#[test]
fn zoom_factor_prompt_retries_invalid_values_and_preserves_model_input() {
    let mut app = test_app();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    for command in ["Point 1,2,3", "Point 4,5,6", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let before = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();

    enter(&mut app, "Zoom Factor");
    assert_eq!(app.zoom_factor_pending, Some(0));
    assert_eq!(app.viewports[0].camera_snapshot(), before[0]);
    for invalid in ["0", "NaN", "abc"] {
        enter(&mut app, invalid);
        assert_eq!(app.zoom_factor_pending, Some(0));
        assert!(app.command_log.back().unwrap().starts_with("Error:"));
        assert_eq!(app.viewports[0].camera_snapshot(), before[0]);
    }
    app.active_viewport = 1;
    enter(&mut app, "2");
    assert_eq!(app.zoom_factor_pending, None);
    assert_ne!(app.viewports[0].camera_snapshot(), before[0]);
    assert_eq!(app.viewports[1].camera_snapshot(), before[1]);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);

    enter(&mut app, "Zoom Factor");
    assert_eq!(app.zoom_factor_pending, Some(1));
    enter(&mut app, "");
    assert_eq!(app.zoom_factor_pending, None);
    assert_eq!(app.viewports[1].camera_snapshot(), before[1]);
    app.active_viewport = 0;
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[0].camera_snapshot(), before[0]);

    let mut selection_app = test_app();
    enter(&mut selection_app, "SelWindow");
    enter(&mut selection_app, "Zoom Factor");
    enter(&mut selection_app, "");
    assert_eq!(selection_app.zoom_factor_pending, None);
    assert_eq!(
        selection_app.selection_window_override,
        Some(viboceros_command::interface::RectSelectionMode::Window)
    );
}

#[test]
fn viewport_navigation_cycles_by_projection_without_changing_cameras_or_model() {
    let mut app = test_app();
    for command in ["Point 1,2,3", "Point 4,5,6", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let cameras = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    for (command, expected) in [
        ("NextViewport", 1),
        ("NextOrthoViewport", 2),
        ("NextPerspectiveViewport", 1),
        ("PrevViewport", 0),
        ("PrevViewport", 3),
        ("NextOrthoViewport", 0),
    ] {
        enter(&mut app, command);
        assert_eq!(app.active_viewport, expected, "{command}");
    }
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::camera_snapshot)
            .collect::<Vec<_>>(),
        cameras
    );
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);

    app.active_viewport = 1;
    enter(&mut app, "SetView World Top");
    let active = app.active_viewport;
    enter(&mut app, "NextPerspectiveViewport");
    assert_eq!(app.active_viewport, active);
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("No matching viewport")
    );
}

#[test]
fn zoom_all_records_one_independent_view_step_per_viewport() {
    let mut app = test_app();
    enter(&mut app, "Point 10,20,30");
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let undo = app.document.undo_label().map(str::to_owned);
    let before = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    enter(&mut app, "ZEA");
    let after = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    for index in 0..4 {
        assert_ne!(after[index], before[index]);
        app.active_viewport = index;
        enter(&mut app, "UndoView");
        assert_eq!(app.viewports[index].camera_snapshot(), before[index]);
        enter(&mut app, "RedoView");
        assert_eq!(app.viewports[index].camera_snapshot(), after[index]);
    }
    assert_eq!(app.document.objects().len(), 1);
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn set_view_world_preserves_framing_and_model_history() {
    use viboceros_command::construction_plane::WorldPlane;

    let mut app = test_app();
    for command in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    app.active_viewport = 2;
    app.viewports[2]
        .plane
        .set(WorldPlane::Right.frame().with_origin(point(17., -11., 3.)));
    let untouched = app.viewports[0].camera_snapshot();
    for (name, kind, plane) in [
        ("Top", ViewKind::Top, Some(WorldPlane::Top)),
        ("Bottom", ViewKind::Bottom, Some(WorldPlane::Bottom)),
        ("Front", ViewKind::Front, Some(WorldPlane::Front)),
        ("Back", ViewKind::Back, Some(WorldPlane::Back)),
        ("Right", ViewKind::Right, Some(WorldPlane::Right)),
        ("Left", ViewKind::Left, Some(WorldPlane::Left)),
        ("Perspective", ViewKind::Perspective, None),
        (
            "TwoPointPerspective",
            ViewKind::Perspective,
            Some(WorldPlane::Top),
        ),
    ] {
        enter(&mut app, "Zoom Factor 2");
        let before = app.viewports[2].camera_snapshot();
        let before_record =
            Viewport::named_view_to_3dm(app.viewports[2].named_view_snapshot(), "Before".into())
                .unwrap();
        let plane_before = app.viewports[2].construction_plane();
        enter(&mut app, &format!("SetView World {name}"));
        assert_eq!(app.viewports[2].kind(), kind);
        let after = app.viewports[2].camera_snapshot();
        let after_record =
            Viewport::named_view_to_3dm(app.viewports[2].named_view_snapshot(), "After".into())
                .unwrap();
        assert_eq!(after_record.target, before_record.target);
        if kind != ViewKind::Perspective {
            assert_eq!(after_record.frustum, before_record.frustum);
        }
        assert_eq!(
            app.viewports[2].construction_plane(),
            plane.map_or(plane_before, |plane| {
                plane.frame().with_origin(plane_before.origin())
            })
        );
        enter(&mut app, "UndoView");
        assert_eq!(app.viewports[2].camera_snapshot(), before);
        enter(&mut app, "RedoView");
        assert_eq!(app.viewports[2].camera_snapshot(), after);
        assert_eq!(app.viewports[0].camera_snapshot(), untouched);
        assert_eq!(app.active_command, pending);
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
}

#[test]
fn world_parallel_command_reports_an_unrenderable_imported_camera_without_mutation() {
    let mut app = test_app();
    for command in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    app.active_viewport = 1;
    let mut camera = Viewport::named_view_to_3dm(
        app.viewports[1].named_view_snapshot(),
        "Very narrow perspective".into(),
    )
    .unwrap();
    // The file is a finite, valid perspective view, but conversion would
    // exceed the renderer's f32 screen-coordinate range.
    camera.frustum = [-1e-300, 1e-300, -1e-300, 1e-300, 1., 1000.];
    app.viewports[1].restore_named_view(Viewport::named_view_from_3dm(&camera).unwrap());
    let before = app.viewports[1].camera_snapshot();
    let plane = app.viewports[1].construction_plane();
    enter(&mut app, "SetView World Top");
    assert_eq!(app.viewports[1].camera_snapshot(), before);
    assert_eq!(app.viewports[1].construction_plane(), plane);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert!(app.command_log.back().unwrap().contains("Error:"));
}

#[test]
fn set_view_cplane_keeps_projection_plane_prompt_and_model_redo() {
    use viboceros_command::construction_plane::WorldPlane;

    let mut app = test_app();
    for command in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    for (index, original_kind) in [(0, ViewKind::Top), (1, ViewKind::Perspective)] {
        app.active_viewport = index;
        app.viewports[index].plane.set(WorldPlane::Right.frame());
        let original_camera = app.viewports[index].camera_snapshot();
        enter(&mut app, "'_SetView _CPlane _Back");
        assert_eq!(
            app.viewports[index].construction_plane(),
            WorldPlane::Right.frame()
        );
        assert!(app.viewports[index].view_label().contains("CPlane Back"));
        assert_eq!(
            app.viewports[index].kind(),
            if original_kind == ViewKind::Perspective {
                ViewKind::Perspective
            } else {
                ViewKind::Plan
            }
        );
        assert_eq!(app.active_command, pending);
        assert_eq!(app.document.redo_label(), redo.as_deref());
        enter(&mut app, "UndoView");
        assert_eq!(app.viewports[index].camera_snapshot(), original_camera);
        assert_eq!(
            app.viewports[index].construction_plane(),
            WorldPlane::Right.frame()
        );
        enter(&mut app, "RedoView");
        assert!(app.viewports[index].view_label().contains("CPlane Back"));
    }
}

#[test]
fn plan_uses_active_cplane_without_consuming_model_prompt_or_redo() {
    use viboceros_command::construction_plane::WorldPlane;

    let mut app = test_app();
    for command in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    app.active_viewport = 1;
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    let untouched = app.viewports[0].camera_snapshot();
    app.viewports[1].plane.set(WorldPlane::Right.frame());
    enter(&mut app, "'_Plan");
    assert_eq!(app.viewports[1].kind(), ViewKind::Plan);
    assert_eq!(
        app.viewports[1].construction_plane(),
        WorldPlane::Right.frame()
    );
    assert_eq!(app.viewports[0].camera_snapshot(), untouched);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[1].kind(), ViewKind::Perspective);
    enter(&mut app, "RedoView");
    assert_eq!(app.viewports[1].kind(), ViewKind::Plan);
}

#[test]
fn zoom_extents_routes_to_the_active_view_without_cancelling_modeling_or_redo() {
    let mut app = test_app();
    for command in ["Point 100,200,300", "Point 110,210,310", "Undo"] {
        enter(&mut app, command);
    }
    app.active_viewport = 1;
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let plane = app.drafting_plane;
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    let redo = app.document.redo_label().map(str::to_owned);
    assert!(redo.is_some());
    enter(&mut app, "ZE");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .starts_with("Zoomed to visible extents")
    );
    assert_eq!(app.active_command, pending);
    assert_eq!(app.drafting_plane, plane);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    assert_eq!(app.document.redo_label(), redo.as_deref());
    let id = app.document.objects().next().unwrap().id();
    app.document
        .select_object(id, viboceros_document::SelectionMode::Replace)
        .unwrap();
    for command in ["Zoom Selected", "ZS"] {
        enter(&mut app, command);
        assert!(
            app.command_log
                .back()
                .unwrap()
                .starts_with("Zoomed to selected visible objects")
        );
        assert_eq!(app.active_command, pending);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), [id]);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), undo.as_deref());
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
    for command in ["Zoom All Extents", "ZEA", "Zoom All Selected", "ZSA"] {
        enter(&mut app, command);
        assert!(app.command_log.back().unwrap().starts_with("Zoomed to"));
        assert!(app.command_log.back().unwrap().ends_with("(all viewports)"));
        assert_eq!(app.active_command, pending);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), [id]);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), undo.as_deref());
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
    for command in [
        "Zoom Factor 2",
        "'_Zoom _Factor 0.5",
        "Zoom In",
        "'_Zoom _Out",
        "Zoom Factor 1",
        "Zoom Factor NaN",
        "Zoom Factor 0",
    ] {
        enter(&mut app, command);
        let message = app.command_log.back().unwrap();
        if command.ends_with("NaN") || command.ends_with(" 0") {
            assert!(message.starts_with("Error:"));
        } else if command.ends_with(" 1") {
            assert!(message.starts_with("Zoom unchanged"));
        } else {
            assert!(message.starts_with("Zoomed by factor"));
        }
        assert_eq!(app.active_viewport, 1);
        assert_eq!(app.active_command, pending);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.document.selected_object_ids().collect::<Vec<_>>(), [id]);
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), undo.as_deref());
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
}

#[test]
fn view_zoom_scale_options_apply_globally_without_editing_the_model() {
    let mut app = test_app();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "Point 2,3,4");
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let before = app.document.objects().cloned().collect::<Vec<_>>();
    enter(&mut app, "Options View Zoom ScaleFactor=1.25");
    assert_eq!(app.zoom_scale, 1.25);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "Zoom In");
    assert!(app.command_log.back().unwrap().contains("factor 0.8"));
    enter(&mut app, "Zoom Out");
    assert!(app.command_log.back().unwrap().contains("factor 1.25"));
    for invalid in [
        "Options View Zoom ScaleFactor=0",
        "Options View Zoom ScaleFactor=5e-324",
    ] {
        enter(&mut app, invalid);
        assert!(app.command_log.back().unwrap().starts_with("Error:"));
        assert_eq!(app.zoom_scale, 1.25);
    }
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), before);
}

#[test]
fn zoom_extents_border_command_preserves_prompt_and_updates_each_projection() {
    let mut app = test_app();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "Point -10,-5,0");
    enter(&mut app, "Point 10,5,0");
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let initial = app.zoom_extents_borders;
    assert_eq!(initial, ZoomExtentsBorders::default());
    enter(&mut app, "SetZoomExtentsBorder");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("ParallelView=1.1 PerspectiveView=1")
    );
    enter(
        &mut app,
        "SetZoomExtentsBorder ParallelView=1.5 PerspectiveView=0.8",
    );
    assert_eq!(app.zoom_extents_borders.parallel, 1.5);
    assert_eq!(app.zoom_extents_borders.perspective, 0.8);
    enter(&mut app, "ZE");
    assert!(
        app.command_log
            .back()
            .unwrap()
            .starts_with("Zoomed to visible")
    );
    enter(&mut app, "ZEA");
    assert!(app.command_log.back().unwrap().ends_with("(all viewports)"));
    enter(&mut app, "SetZoomExtentsBorder PerspectiveView=0");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(app.zoom_extents_borders.perspective, 0.8);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
}
