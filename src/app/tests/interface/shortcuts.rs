use super::*;

#[test]
fn zoom_shortcuts_preserve_focused_modeling_input_and_consume_repeats() {
    let mut app = test_app();
    let context = egui::Context::default();
    for command in [
        "Point 100,200,300",
        "Point 110,210,310",
        "Undo",
        "Line",
        "0",
    ] {
        enter(&mut app, command);
    }
    layout_viewports(&context, &mut app);
    app.command_input = "r1.5,".into();
    app.command_focus_requested = true;
    frame(&context, &mut app, 1000.0, vec![])
        .1
        .drop_without_applying_deltas();
    let focused = context.memory(|memory| memory.focused());
    assert!(context.text_edit_focused());
    let pending = app.active_command;
    let plane = app.drafting_plane;
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    let redo = app.document.redo_label().map(str::to_owned);
    for platform in [egui::Modifiers::CTRL, egui::Modifiers::MAC_CMD] {
        for (extra, suffix) in [
            (egui::Modifiers::SHIFT, "(active viewport)"),
            (egui::Modifiers::ALT, "(all viewports)"),
        ] {
            let modifiers = egui::Modifiers::COMMAND | platform | extra;
            frame(
                &context,
                &mut app,
                1000.0,
                vec![key(egui::Key::E, modifiers, true, false)],
            )
            .1
            .drop_without_applying_deltas();
            assert!(
                app.command_log
                    .back()
                    .unwrap()
                    .starts_with("Zoomed to visible extents")
            );
            assert!(app.command_log.back().unwrap().ends_with(suffix));
            let log = app.command_log.clone();
            // egui derives repeats from held state, even when RawInput says false.
            frame(
                &context,
                &mut app,
                1000.0,
                vec![key(egui::Key::E, modifiers, true, false)],
            )
            .1
            .drop_without_applying_deltas();
            assert_eq!(app.command_log, log);
            frame(
                &context,
                &mut app,
                1000.0,
                vec![key(egui::Key::E, modifiers, false, false)],
            )
            .1
            .drop_without_applying_deltas();
            assert_eq!(app.command_log, log);
            assert_eq!(app.command_input, "r1.5,");
            assert_eq!(app.active_command, pending);
            assert_eq!(app.drafting_plane, plane);
            assert_eq!(context.memory(|memory| memory.focused()), focused);
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
            assert_eq!(app.document.undo_label(), undo.as_deref());
            assert_eq!(app.document.redo_label(), redo.as_deref());
        }
    }
    let log = app.command_log.clone();
    let modifiers = egui::Modifiers::COMMAND
        | egui::Modifiers::CTRL
        | egui::Modifiers::SHIFT
        | egui::Modifiers::ALT;
    frame(
        &context,
        &mut app,
        1000.0,
        vec![key(egui::Key::E, modifiers, true, false)],
    )
    .1
    .drop_without_applying_deltas();
    assert_eq!(app.command_log, log);

    for platform in [egui::Modifiers::CTRL, egui::Modifiers::MAC_CMD] {
        let modifiers = egui::Modifiers::COMMAND | platform;
        frame(
            &context,
            &mut app,
            1000.0,
            vec![key(egui::Key::W, modifiers, true, false)],
        )
        .1
        .drop_without_applying_deltas();
        assert!(app.zoom_window_pending);
        assert_eq!(app.command_input, "r1.5,");
        assert_eq!(app.active_command, pending);
        assert_eq!(context.memory(|memory| memory.focused()), focused);
        frame(
            &context,
            &mut app,
            1000.0,
            vec![key(egui::Key::W, modifiers, false, false)],
        )
        .1
        .drop_without_applying_deltas();
    }
}

#[test]
fn home_end_view_history_shortcuts_leave_text_editing_keys_alone() {
    let mut app = test_app();
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let original = app.viewports[0].camera_snapshot();
    enter(&mut app, "Zoom Factor 2");
    let zoomed = app.viewports[0].camera_snapshot();
    app.command_input = "r1.5,".into();
    app.command_focus_requested = true;
    frame(&context, &mut app, 1000.0, vec![])
        .1
        .drop_without_applying_deltas();
    assert!(context.text_edit_focused());
    for key_code in [egui::Key::Home, egui::Key::End] {
        frame(
            &context,
            &mut app,
            1000.0,
            vec![key(key_code, egui::Modifiers::NONE, true, false)],
        )
        .1
        .drop_without_applying_deltas();
        frame(
            &context,
            &mut app,
            1000.0,
            vec![key(key_code, egui::Modifiers::NONE, false, false)],
        )
        .1
        .drop_without_applying_deltas();
    }
    assert_eq!(app.viewports[0].camera_snapshot(), zoomed);
    assert_eq!(app.command_input, "r1.5,");

    let unfocused = egui::Context::default();
    for (key_code, expected) in [(egui::Key::Home, original), (egui::Key::End, zoomed)] {
        unfocused
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events: vec![key(key_code, egui::Modifiers::NONE, true, false)],
                    ..Default::default()
                },
                |ui| app.handle_interface_shortcuts(ui),
            )
            .drop_without_applying_deltas();
        assert_eq!(app.viewports[0].camera_snapshot(), expected);
    }
}

#[test]
fn interface_shortcuts_work_with_command_focus_and_preserve_text_and_prompt() {
    let mut app = test_app();
    let context = egui::Context::default();
    for command in ["Line", "0"] {
        enter(&mut app, command);
    }
    app.command_input = "w2,".into();
    app.command_focus_requested = true;
    frame(&context, &mut app, 1000.0, vec![])
        .1
        .drop_without_applying_deltas();
    let focused = context.memory(|memory| memory.focused());
    assert!(context.text_edit_focused());
    let pending = app.active_command;
    let command_alt = egui::Modifiers::COMMAND | egui::Modifiers::CTRL | egui::Modifiers::ALT;
    for (key_code, modifiers) in [
        (egui::Key::F9, egui::Modifiers::NONE),
        (egui::Key::F4, egui::Modifiers::NONE),
        (egui::Key::S, command_alt),
    ] {
        frame(
            &context,
            &mut app,
            1000.0,
            vec![
                key(key_code, modifiers, true, false),
                key(key_code, modifiers, false, false),
            ],
        )
        .1
        .drop_without_applying_deltas();
        assert_eq!(app.active_command, pending);
        assert_eq!(app.command_input, "w2,");
        assert_eq!(context.memory(|memory| memory.focused()), focused);
    }
    assert!(!app.grid_snap && !app.osnap);
    assert_eq!(app.viewports[0].display_mode, DisplayMode::Shaded);
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Wireframe);
    let mac_alt = egui::Modifiers::COMMAND | egui::Modifiers::MAC_CMD | egui::Modifiers::ALT;
    frame(
        &context,
        &mut app,
        1000.0,
        vec![
            key(egui::Key::G, mac_alt, true, false),
            key(egui::Key::G, mac_alt, false, false),
        ],
    )
    .1
    .drop_without_applying_deltas();
    assert_eq!(app.viewports[0].display_mode, DisplayMode::Ghosted);
}

#[test]
fn viewport_cycle_shortcuts_preserve_focused_command_text() {
    let mut app = test_app();
    let context = egui::Context::default();
    enter(&mut app, "Line");
    app.command_input = "1,".into();
    app.command_focus_requested = true;
    frame(&context, &mut app, 1000.0, vec![])
        .1
        .drop_without_applying_deltas();
    let focused = context.memory(|memory| memory.focused());
    let pending = app.active_command;
    for (modifiers, expected) in [
        (egui::Modifiers::COMMAND | egui::Modifiers::CTRL, 1),
        (
            egui::Modifiers::COMMAND | egui::Modifiers::MAC_CMD | egui::Modifiers::SHIFT,
            0,
        ),
    ] {
        frame(
            &context,
            &mut app,
            1000.0,
            vec![
                key(egui::Key::Tab, modifiers, true, false),
                key(egui::Key::Tab, modifiers, false, false),
            ],
        )
        .1
        .drop_without_applying_deltas();
        assert_eq!(app.active_viewport, expected);
        assert_eq!(app.command_input, "1,");
        assert_eq!(app.active_command, pending);
        assert_eq!(context.memory(|memory| memory.focused()), focused);
    }
}

#[test]
fn interface_shortcuts_ignore_repeats_unrelated_keys_and_extra_modifiers() {
    let mut app = test_app();
    let context = egui::Context::default();
    // egui derives repeat from its held-key state, not the RawInput repeat flag.
    frame(
        &context,
        &mut app,
        1000.0,
        vec![key(egui::Key::F9, egui::Modifiers::NONE, true, false)],
    )
    .1
    .drop_without_applying_deltas();
    let state = app.interface_state();
    let events = vec![
        key(egui::Key::F9, egui::Modifiers::NONE, true, true),
        key(egui::Key::F4, egui::Modifiers::ALT, true, false),
        key(egui::Key::F9, egui::Modifiers::SHIFT, true, false),
        key(egui::Key::F3, egui::Modifiers::NONE, true, false),
        key(egui::Key::F11, egui::Modifiers::NONE, true, false),
        key(egui::Key::S, egui::Modifiers::COMMAND, true, false),
    ];
    frame(&context, &mut app, 1000.0, events)
        .1
        .drop_without_applying_deltas();
    assert_eq!(app.interface_state(), state);
}

#[test]
fn interface_shortcuts_preserve_event_order_within_one_frame() {
    let mut app = test_app();
    let context = egui::Context::default();
    let modifiers = egui::Modifiers::COMMAND | egui::Modifiers::CTRL | egui::Modifiers::ALT;
    frame(
        &context,
        &mut app,
        1000.0,
        vec![
            key(egui::Key::G, modifiers, true, false),
            key(egui::Key::W, modifiers, true, false),
        ],
    )
    .1
    .drop_without_applying_deltas();
    assert_eq!(app.viewports[0].display_mode, DisplayMode::Wireframe);
    assert_eq!(app.command_log.len(), 2);
    assert!(app.command_log[0].contains("Ghosted"));
    assert!(app.command_log[1].contains("Wireframe"));
}

#[test]
fn plane_history_shortcuts_preserve_drafting_and_do_not_steal_text_selection_keys() {
    let mut app = test_app();
    let context = egui::Context::default();
    for command in ["Line", "0", "CPlane World Front", "CPlane Elevation 2"] {
        enter(&mut app, command);
    }
    let before = app.viewports[0].construction_plane();
    let pending = app.active_command;
    app.command_input = "r2,".into();
    frame(
        &context,
        &mut app,
        1000.,
        vec![
            key(egui::Key::Home, egui::Modifiers::SHIFT, true, false),
            key(egui::Key::Home, egui::Modifiers::SHIFT, false, false),
        ],
    )
    .1
    .drop_without_applying_deltas();
    assert_ne!(app.viewports[0].construction_plane(), before);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.command_input, "r2,");
    frame(
        &context,
        &mut app,
        1000.,
        vec![
            key(egui::Key::End, egui::Modifiers::SHIFT, true, false),
            key(egui::Key::End, egui::Modifiers::SHIFT, false, false),
        ],
    )
    .1
    .drop_without_applying_deltas();
    assert_eq!(app.viewports[0].construction_plane(), before);
    app.command_focus_requested = true;
    frame(&context, &mut app, 1000., vec![])
        .1
        .drop_without_applying_deltas();
    assert!(context.text_edit_focused());
    frame(
        &context,
        &mut app,
        1000.,
        vec![
            key(egui::Key::Home, egui::Modifiers::SHIFT, true, false),
            key(egui::Key::Home, egui::Modifiers::SHIFT, false, false),
        ],
    )
    .1
    .drop_without_applying_deltas();
    assert_eq!(app.viewports[0].construction_plane(), before);
    assert_eq!(app.command_input, "r2,");
}
