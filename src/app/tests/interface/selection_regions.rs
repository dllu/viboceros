use super::*;

#[test]
fn volume_sphere_picks_model_points_and_accepts_mode_changes() {
    let mut app = test_app();
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Point 5,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "SelVolumeSphere SelectionMode=Window");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelVolumeSphere {
            center: None,
            mode: RectSelectionMode::Window
        })
    ));
    assert!(app.accept_drafting_point(Point3::try_new(0.0, 0.0, 0.0).unwrap()));
    app.command_input = "SelectionMode=InvertWindow".into();
    app.run_command_input();
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelVolumeSphere {
            center: Some(_),
            mode: RectSelectionMode::InvertWindow
        })
    ));
    assert!(app.accept_drafting_point(Point3::try_new(1.0, 0.0, 0.0).unwrap()));
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1]]
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());

    enter(&mut app, "SelVolumeSphere 0,0,0 1 SelectionMode=Window");
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[0]]
    );
}

#[test]
fn volume_pipe_picks_curve_then_radius_and_accepts_mode_changes() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    enter(&mut app, "Point 5,0.5,0");
    enter(&mut app, "Point 5,2,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "SelVolumePipe SelectionMode=Window");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelVolumePipe {
            source: None,
            mode: RectSelectionMode::Window
        })
    ));
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelVolumePipe {
            source: Some(_),
            mode: RectSelectionMode::Window
        })
    ));
    app.command_input = "SelectionMode=Crossing".into();
    app.run_command_input();
    assert!(app.accept_drafting_point(Point3::try_new(5.0, 1.0, 0.0).unwrap()));
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1]]
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());

    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "SelVolumePipe");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelVolumePipe {
            source: Some(source),
            ..
        }) if source == ids[0]
    ));
}

#[test]
fn volume_object_picks_closed_mesh_and_accepts_mode_changes() {
    let mut app = test_app();
    enter(&mut app, "MeshBox 0,0,0 2,2,0 2");
    enter(&mut app, "Point 1,1,1");
    enter(&mut app, "Line -1,1,1 3,1,1");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "SelVolumeObject SelectionMode=Crossing");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelVolumeObject {
            mode: RectSelectionMode::Crossing
        })
    ));
    app.command_input = "SelectionMode=Window".into();
    app.run_command_input();
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1]]
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());

    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "SelVolumeObject");
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![ids[1], ids[2]]
    );
}

#[test]
fn sel_box_picks_base_and_height_without_creating_geometry() {
    let mut app = test_app();
    enter(&mut app, "Point 1,1,1");
    enter(&mut app, "Point 4,1,1");
    let inside = app.document.objects().next().unwrap().id();
    let original = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "SelBox SelectionMode=InvertWindow");
    for coordinates in [(0.0, 0.0, 0.0), (2.0, 2.0, 0.0)] {
        assert!(app.accept_drafting_point(
            Point3::try_new(coordinates.0, coordinates.1, coordinates.2).unwrap()
        ));
    }
    app.command_input = "SelectionMode=Window".into();
    app.run_command_input();
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::SelBox {
            opposite: Some(_),
            mode: RectSelectionMode::Window,
            ..
        })
    ));
    assert!(app.accept_drafting_point(Point3::try_new(0.0, 0.0, 2.0).unwrap()));
    assert!(app.active_command.is_none());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        vec![inside]
    );
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        original
    );
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn typed_window_commands_force_mode_for_one_drag_and_preserve_model_history() {
    let mut app = test_app();
    enter(&mut app, "Point 0,0,0");
    enter(&mut app, "Line -5,0,0 5,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    let context = egui::Context::default();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    for (command, start, end, crossing) in [
        (
            "C",
            egui::Pos2::new(390., 290.),
            egui::Pos2::new(410., 310.),
            true,
        ),
        (
            "SelWindow",
            egui::Pos2::new(410., 310.),
            egui::Pos2::new(390., 290.),
            false,
        ),
    ] {
        app.document.clear_selection();
        enter(&mut app, command);
        assert_eq!(
            app.selection_window_override,
            Some(if crossing {
                viboceros_command::interface::RectSelectionMode::Crossing
            } else {
                viboceros_command::interface::RectSelectionMode::Window
            })
        );
        selection_capture_frame(&context, &mut app, vec![]);
        selection_capture_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        selection_capture_frame(&context, &mut app, vec![egui::Event::PointerMoved(end)]);
        let output = selection_capture_frame(&context, &mut app, vec![button(end, false)]);
        assert!(output.selection_click.is_none());
        assert_eq!(output.selection_window.as_ref().unwrap().crossing, crossing);
        assert!(app.handle_viewport_action(output));
        assert_eq!(app.selection_window_override, None);
        assert!(app.document.is_selected(ids[0]));
        assert_eq!(app.document.is_selected(ids[1]), crossing);
        assert_eq!(app.document.undo_label(), undo.as_deref());
    }
    for (start, end, crossing) in [
        (
            egui::Pos2::new(390., 290.),
            egui::Pos2::new(410., 310.),
            false,
        ),
        (
            egui::Pos2::new(410., 310.),
            egui::Pos2::new(390., 290.),
            true,
        ),
    ] {
        app.document.clear_selection();
        enter(&mut app, "SelRectangular");
        assert_eq!(
            app.selection_window_override,
            Some(viboceros_command::interface::RectSelectionMode::Automatic)
        );
        selection_capture_frame(&context, &mut app, vec![]);
        selection_capture_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        selection_capture_frame(&context, &mut app, vec![egui::Event::PointerMoved(end)]);
        let output = selection_capture_frame(&context, &mut app, vec![button(end, false)]);
        assert_eq!(output.selection_window.as_ref().unwrap().crossing, crossing);
        assert!(app.handle_viewport_action(output));
        assert!(app.document.is_selected(ids[0]));
        assert_eq!(app.document.is_selected(ids[1]), crossing);
    }
}

#[test]
fn typed_window_selection_can_feed_an_object_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line -1,0,0 1,0,0");
    let id = app.document.objects().next().unwrap().id();
    enter(&mut app, "Flip");
    assert!(app.object_prompt.is_some());
    enter(&mut app, "W");
    assert_eq!(
        app.selection_window_override,
        Some(viboceros_command::interface::RectSelectionMode::Window)
    );
    let context = egui::Context::default();
    let start = egui::Pos2::new(300., 250.);
    let end = egui::Pos2::new(500., 350.);
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
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    selection_capture_frame(&context, &mut app, vec![egui::Event::PointerMoved(end)]);
    let output = selection_capture_frame(&context, &mut app, vec![button(end, false)]);
    assert!(app.handle_viewport_action(output));
    assert!(app.document.is_selected(id));
    enter(&mut app, "");
    let Geometry::Line(line) = app.document.object(id).unwrap().geometry() else {
        panic!("expected line")
    };
    assert_eq!(line.start(), point(1., 0., 0.));
}

#[test]
fn rectangular_inverse_modes_distinguish_partial_overlap_from_fully_outside() {
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
    let context = egui::Context::default();
    let start = egui::Pos2::new(390., 290.);
    let end = egui::Pos2::new(410., 310.);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    };
    for (index, (name, mode, expected)) in [
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
            enter(&mut app, "SelRectangular");
            enter(&mut app, &format!("SelectionMode={name}"));
        } else {
            enter(&mut app, &format!("SelRectangular SelectionMode={name}"));
        }
        assert_eq!(app.selection_window_override, Some(mode));
        selection_capture_frame(&context, &mut app, vec![]);
        selection_capture_frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        selection_capture_frame(&context, &mut app, vec![egui::Event::PointerMoved(end)]);
        let output = selection_capture_frame(&context, &mut app, vec![button(end, false)]);
        assert!(output.selection_window.as_ref().unwrap().inverted);
        assert!(app.handle_viewport_action(output));
        assert_eq!(app.selection_window_override, None);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn empty_enter_cancels_typed_window_without_advancing_an_object_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line -1,0,0 1,0,0");
    enter(&mut app, "Flip");
    let before = app.object_prompt.clone();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "SelCrossing");
    assert_eq!(
        app.selection_window_override,
        Some(viboceros_command::interface::RectSelectionMode::Crossing)
    );
    enter(&mut app, "");
    assert_eq!(app.selection_window_override, None);
    assert_eq!(app.object_prompt, before);
    assert_eq!(app.document.undo_label(), undo.as_deref());
}

#[test]
fn typed_window_alias_does_not_replace_point_input() {
    let mut app = test_app();
    enter(&mut app, "Line");
    let pending = app.active_command;
    enter(&mut app, "W");
    assert_eq!(app.selection_window_override, None);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().count(), 0);
    enter(&mut app, "C");
    assert_eq!(app.selection_window_override, None);
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Circle { .. })
    ));
}
