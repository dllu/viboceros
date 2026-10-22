use super::*;

#[test]
fn fence_command_collects_one_viewport_polyline_and_selects_only_crossed_objects() {
    let mut app = test_app();
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Point 0,1,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    assert_eq!(
        interface::parse("_SelFence"),
        Some(Ok(InterfaceCommand::SelFence))
    );
    assert_eq!(
        interface::parse("SelFence Curve"),
        Some(Ok(InterfaceCommand::SelFenceCurve))
    );
    enter(&mut app, "SelFence");
    let context = egui::Context::default();
    let click = |point, pressed| egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let first = egui::Pos2::new(380.0, 300.0);
    let second = egui::Pos2::new(420.0, 300.0);
    selection_capture_frame(&context, &mut app, Vec::new());
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(first), click(first, true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![click(first, false)]);
    assert_eq!(output.fence_point, Some((first, 0, SelectionMode::Replace)));
    assert!(app.handle_viewport_action(output));
    app.add_fence_point(second, 1, SelectionMode::Replace);
    assert_eq!(app.fence_selection.as_ref().unwrap().points.len(), 1);
    let right = |pressed| egui::Event::PointerButton {
        pos: second,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    selection_capture_frame(&context, &mut app, vec![right(true)]);
    let output = selection_capture_frame(&context, &mut app, vec![right(false)]);
    assert!(output.enter_pressed);
    assert!(app.handle_viewport_action(output));
    assert!(app.fence_selection.is_some());
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(second), click(second, true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![click(second, false)]);
    assert_eq!(
        output.fence_point,
        Some((second, 0, SelectionMode::Replace))
    );
    assert!(app.handle_viewport_action(output));
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "");
    assert!(app.fence_selection.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[0]]
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn fence_selection_feeds_an_existing_object_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line -1,0,0 1,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Flip");
    let prompt = app.object_prompt.clone();
    enter(&mut app, "SelFence");
    assert_eq!(app.object_prompt, prompt);
    layout_viewports(&egui::Context::default(), &mut app);
    app.add_fence_point(egui::Pos2::new(400.0, 290.0), 0, SelectionMode::Replace);
    app.add_fence_point(egui::Pos2::new(400.0, 310.0), 0, SelectionMode::Replace);
    enter(&mut app, "");
    assert!(app.document.is_selected(id));
    assert!(app.object_prompt.is_some());
    enter(&mut app, "");
    let Geometry::Line(line) = app.document.object(id).unwrap().geometry() else {
        panic!("expected line");
    };
    assert_eq!(line.start(), point(1.0, 0.0, 0.0));
}

#[test]
fn fence_command_keeps_accepted_points_aligned_after_pan() {
    let mut app = test_app();
    let target = app
        .document
        .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
        .unwrap();
    app.document
        .add_geometry(Geometry::Point(point(-1.25, 0.0, 0.0)))
        .unwrap();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "SelFence");
    app.add_fence_point(egui::Pos2::new(380.0, 300.0), 0, SelectionMode::Replace);
    let middle = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Middle,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let start = egui::Pos2::new(300.0, 200.0);
    let finish = egui::Pos2::new(340.0, 200.0);
    selection_capture_frame(&context, &mut app, vec![]);
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(start), middle(start, true)],
    );
    selection_capture_frame(&context, &mut app, vec![egui::Event::PointerMoved(finish)]);
    selection_capture_frame(&context, &mut app, vec![middle(finish, false)]);
    app.add_fence_point(egui::Pos2::new(460.0, 300.0), 0, SelectionMode::Replace);
    enter(&mut app, "");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![target]
    );
}

#[test]
fn fence_curve_option_picks_a_visible_curve_without_selecting_it() {
    let mut app = test_app();
    enter(&mut app, "Line -2,0,0 2,0,0");
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Point 0,1,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "SelFence");
    enter(&mut app, "Curve");
    assert!(app.fence_selection.as_ref().unwrap().curve_pick);
    let context = egui::Context::default();
    let pointer = egui::Pos2::new(460.0, 300.0);
    let click = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    selection_capture_frame(&context, &mut app, vec![]);
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(pointer), click(true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![click(false)]);
    assert_eq!(
        output.selection_click,
        Some(SelectionClick {
            object_id: Some(ids[0]),
            mode: SelectionMode::Replace,
        })
    );
    assert!(app.handle_viewport_action(output));
    assert!(app.fence_selection.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1]]
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn fence_curve_choice_menu_routes_the_chosen_curve_to_the_fence() {
    let mut app = test_app();
    enter(&mut app, "Line -2,0,0 2,0,0");
    enter(&mut app, "Line -2,0,0 2,0,0");
    enter(&mut app, "Point 0,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    enter(&mut app, "SelFence Curve");
    let context = egui::Context::default();
    let pointer = egui::Pos2::new(460.0, 300.0);
    let click = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    selection_capture_frame(&context, &mut app, vec![]);
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(pointer), click(true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![click(false)]);
    assert!(output.selection_click.is_none());
    assert_eq!(
        output.selection_choice.as_ref().unwrap().object_ids,
        ids[..2]
    );
    assert!(app.handle_viewport_action(output));
    assert_eq!(app.document.selected_object_count(), 0);
    app.selection_menu = None;
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[0], ids[2]]
    );
    assert!(app.fence_selection.is_none());
}

#[test]
fn boundary_command_selects_from_closed_curve_and_keeps_prompt_on_open_curve() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "Line 4,0,0 5,0,0");
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Point 5,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    assert_eq!(
        interface::parse("SelBoundary SelectionMode=InvertCrossing"),
        Some(Ok(InterfaceCommand::SelBoundary(
            RectSelectionMode::InvertCrossing
        )))
    );
    enter(&mut app, "SelBoundary");
    assert_eq!(app.boundary_selection, Some(RectSelectionMode::Crossing));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[1]),
        mode: SelectionMode::Replace,
    });
    assert!(app.boundary_selection.is_some());
    app.command_input = "SelectionMode=InvertWindow".into();
    app.run_command_input();
    assert_eq!(
        app.boundary_selection,
        Some(RectSelectionMode::InvertWindow)
    );
    let context = egui::Context::default();
    selection_capture_frame(&context, &mut app, vec![]);
    let pointer = egui::Pos2::new(480.0, 300.0);
    let click = |pressed| egui::Event::PointerButton {
        pos: pointer,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(pointer), click(true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![click(false)]);
    assert_eq!(
        output
            .selection_click
            .as_ref()
            .and_then(|click| click.object_id),
        Some(ids[0])
    );
    assert!(app.handle_viewport_action(output));
    assert!(app.boundary_selection.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1], ids[3]]
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn boundary_command_accepts_preselected_closed_curve() {
    let mut app = test_app();
    enter(&mut app, "Circle 0,0,0 2");
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Point 5,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let context = egui::Context::default();
    selection_capture_frame(&context, &mut app, vec![]);
    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "SelBoundary SelectionMode=Window");
    assert!(app.boundary_selection.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1]]
    );
}

#[test]
fn lasso_click_path_supports_undo_and_prompt_selection() {
    let mut app = test_app();
    enter(&mut app, "Point 0,0,0");
    let id = app.document.objects().next().unwrap().id();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "Lasso SelectionMode=Window");
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    for point in [
        egui::Pos2::new(380.0, 280.0),
        egui::Pos2::new(420.0, 280.0),
        egui::Pos2::new(420.0, 320.0),
        egui::Pos2::new(380.0, 320.0),
    ] {
        selection_capture_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(point), button(point, true)],
        );
        let output = selection_capture_frame(&context, &mut app, vec![button(point, false)]);
        assert!(output.selection_click.is_none());
        assert!(output.lasso_point.is_some());
        app.handle_viewport_action(output);
    }
    assert_eq!(app.lasso_selection.as_ref().unwrap().points.len(), 4);
    enter(&mut app, "Undo");
    assert_eq!(app.lasso_selection.as_ref().unwrap().points.len(), 3);
    enter(&mut app, "SelectionMode=Crossing");
    assert_eq!(
        app.lasso_selection.as_ref().unwrap().mode,
        RectSelectionMode::Crossing
    );
    enter(&mut app, "");
    assert!(app.lasso_selection.is_none());
    assert!(app.document.is_selected(id));
}

#[test]
fn lasso_freehand_drag_selects_once_and_clears_capture() {
    let mut app = test_app();
    enter(&mut app, "Point 0,0,0");
    let id = app.document.objects().next().unwrap().id();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "Lasso");
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    let start = egui::Pos2::new(380.0, 280.0);
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    for point in [
        egui::Pos2::new(420.0, 280.0),
        egui::Pos2::new(420.0, 320.0),
        egui::Pos2::new(380.0, 320.0),
    ] {
        selection_capture_frame(&context, &mut app, vec![egui::Event::PointerMoved(point)]);
    }
    let output = selection_capture_frame(&context, &mut app, vec![button(start, false)]);
    assert!(output.lasso_stroke.is_some());
    assert!(output.selection_window.is_none());
    app.handle_viewport_action(output);
    assert!(app.document.is_selected(id));
    assert!(app.lasso_selection.is_none());
}

#[test]
fn lasso_can_supply_objects_to_an_active_command_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line -1,0,0 1,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Flip");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "Lasso SelectionMode=Window");
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    for point in [
        egui::Pos2::new(300.0, 250.0),
        egui::Pos2::new(500.0, 250.0),
        egui::Pos2::new(500.0, 350.0),
        egui::Pos2::new(300.0, 350.0),
    ] {
        app.accept_lasso_point(point, 0, SelectionMode::Replace);
    }
    app.finish_lasso_selection();
    assert!(app.lasso_selection.is_none());
    assert!(app.document.is_selected(id));
    enter(&mut app, "");
    let Geometry::Line(line) = app.document.object(id).unwrap().geometry() else {
        panic!("expected line")
    };
    assert_eq!(line.start(), point(1.0, 0.0, 0.0));
}

#[test]
fn circular_selection_modes_use_two_viewport_clicks_and_preserve_history() {
    use viboceros_command::interface::RectSelectionMode;
    let mut app = test_app();
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Line -5,0,0 5,0,0");
    enter(&mut app, "Point 10,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    let context = egui::Context::default();
    let center = egui::Pos2::new(400., 300.);
    let edge = egui::Pos2::new(420., 300.);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    for (index, (name, mode, expected)) in [
        ("Window", RectSelectionMode::Window, vec![ids[0]]),
        (
            "Crossing",
            RectSelectionMode::Crossing,
            vec![ids[0], ids[1]],
        ),
        (
            "InvertWindow",
            RectSelectionMode::InvertWindow,
            vec![ids[2]],
        ),
        (
            "InvertCrossing",
            RectSelectionMode::InvertCrossing,
            vec![ids[1], ids[2]],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        app.document.clear_selection();
        if index == 0 {
            enter(&mut app, "SelCircular");
            enter(&mut app, "SelectionMode=Window");
        } else {
            enter(&mut app, &format!("SelCircular SelectionMode={name}"));
        }
        assert_eq!(
            app.circular_selection,
            Some(CircularSelectionState::PickCenter(mode))
        );
        selection_capture_frame(&context, &mut app, vec![]);
        selection_capture_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(center), button(center, true)],
        );
        let output = selection_capture_frame(&context, &mut app, vec![button(center, false)]);
        assert!(output.selection_click.is_none());
        assert_eq!(output.circular_center_pick, Some((center, 0)));
        assert!(app.handle_viewport_action(output));
        assert_eq!(
            app.circular_selection,
            Some(CircularSelectionState::PickRadius {
                mode,
                center,
                viewport: 0
            })
        );
        selection_capture_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(edge), button(edge, true)],
        );
        let output = selection_capture_frame(&context, &mut app, vec![button(edge, false)]);
        assert_eq!(
            output.selection_window.as_ref().unwrap().inverted,
            mode.inverted()
        );
        assert!(app.handle_viewport_action(output));
        assert_eq!(app.circular_selection, None);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(app.document.undo_label(), undo.as_deref());
    }
}

#[test]
fn circular_selection_feeds_an_object_prompt_and_can_be_canceled() {
    let mut app = test_app();
    enter(&mut app, "Line -1,0,0 1,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Flip");
    let prompt = app.object_prompt.clone();
    enter(&mut app, "SelCircular");
    assert!(app.circular_selection.is_some());
    enter(&mut app, "");
    assert_eq!(app.circular_selection, None);
    assert_eq!(app.object_prompt, prompt);
    enter(&mut app, "SelCircular");
    let context = egui::Context::default();
    let center = egui::Pos2::new(400., 300.);
    let edge = egui::Pos2::new(420., 300.);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    selection_capture_frame(&context, &mut app, vec![]);
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(center), button(center, true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![button(center, false)]);
    assert!(app.handle_viewport_action(output));
    selection_capture_frame(
        &context,
        &mut app,
        vec![egui::Event::PointerMoved(edge), button(edge, true)],
    );
    let output = selection_capture_frame(&context, &mut app, vec![button(edge, false)]);
    assert!(app.handle_viewport_action(output));
    assert!(app.document.is_selected(id));
    enter(&mut app, "");
    let Geometry::Line(line) = app.document.object(id).unwrap().geometry() else {
        panic!("expected line")
    };
    assert_eq!(line.start(), point(1., 0., 0.));
}
