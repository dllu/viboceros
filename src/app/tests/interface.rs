use super::*;
use viboceros_command::interface::{self, InterfaceCommand, SwitchAction};

fn enter(app: &mut VibocerosApp, command: &str) {
    app.command_input = command.into();
    app.run_command();
}

#[test]
fn zoom_extents_routes_to_the_active_view_without_cancelling_modeling_or_redo() {
    let mut app = test_app();
    for command in ["Point 100,200,300", "Point 110,210,310", "Undo"] {
        enter(&mut app, command);
    }
    app.active_viewport = 1;
    let context = egui::Context::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                app.viewports[1].show(ui, &app.document, ViewportInput::default(), &[], 1, true);
            },
        )
        .drop_without_applying_deltas();
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
}

#[test]
fn tolerance_command_updates_settings_through_application_history() {
    let mut app = test_app();
    let initial = app.document.tolerance();
    enter(&mut app, "Tolerance Absolute=0.01 AngleDegrees=1");
    assert_eq!(app.document.tolerance().absolute(), 0.01);
    assert_eq!(app.document.tolerance().angular(), 1.0_f64.to_radians());
    assert_eq!(app.document.tolerance().relative(), initial.relative());
    enter(&mut app, "Undo");
    assert_eq!(app.document.tolerance(), initial);
    let before = format!("{:?}", app.document);
    enter(&mut app, "Tolerance");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.tolerance().absolute(), 0.01);
}

#[test]
fn units_command_routes_through_the_application_and_preserves_query_redo() {
    use viboceros_geometry::LengthUnitSystem;
    let mut app = test_app();
    enter(&mut app, "Point 1000,2000,3000");
    enter(&mut app, "Units Meters Scale=Yes");
    assert_eq!(app.document.units(), &LengthUnitSystem::Meters);
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::Point(Point3::try_new(1.0, 2.0, 3.0).unwrap())
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.units(), &LengthUnitSystem::Millimeters);
    let before = format!("{:?}", app.document);
    enter(&mut app, "Units");
    assert_eq!(format!("{:?}", app.document), before);
    enter(&mut app, "Redo");
    assert_eq!(app.document.units(), &LengthUnitSystem::Meters);
    enter(
        &mut app,
        "Units Custom MetersPerUnit=0.25 Scale=Yes Name=fixture  尺",
    );
    assert_eq!(
        app.document.units(),
        &LengthUnitSystem::Custom {
            name: "fixture  尺".into(),
            meters_per_unit: 0.25
        }
    );
    assert_eq!(
        app.document.objects().next().unwrap().geometry(),
        &Geometry::Point(Point3::try_new(4.0, 8.0, 12.0).unwrap())
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.units(), &LengthUnitSystem::Meters);
}

#[test]
fn interface_commands_preserve_a_front_view_polyline_and_one_model_undo_step() {
    let mut app = test_app();
    app.active_viewport = 2;
    for command in ["Point 7,8,9", "SelAll", "Polyline", "1,2"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let plane = app.drafting_plane;
    let points = app.curve_points.clone();
    let last = app.last_point;
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    for command in [
        "Snap",
        "SetSnap On",
        "'DisableOsnap Disable",
        "SmartTrack Off",
        "-_SetDisplayMode Viewport=All Mode=Ghosted",
        "Help",
        "Help UI",
    ] {
        enter(&mut app, command);
        assert_eq!(app.active_command, pending, "{command}");
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.last_point, last);
        assert_eq!(app.curve_points, points);
        assert_eq!(
            app.document.selected_object_ids().collect::<Vec<_>>(),
            selected
        );
        assert_eq!(app.document.objects().len(), 1);
        assert!(app.command_input.is_empty());
    }
    assert!(app.grid_snap && !app.osnap && !app.smart_track);
    assert!(
        app.viewports
            .iter()
            .all(|v| v.display_mode == DisplayMode::Ghosted)
    );
    enter(&mut app, "4,5");
    enter(&mut app, "");
    assert!(app.active_command.is_none());
    let Geometry::Polyline(polyline) = app.document.objects().last().unwrap().geometry() else {
        panic!("polyline")
    };
    assert_eq!(
        polyline.vertices(),
        &[point(1.0, 0.0, 2.0), point(4.0, 0.0, 5.0)]
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 1);
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().len(), 0);
    assert!(!app.document.can_undo());
    assert!(!app.osnap && !app.smart_track);
}

#[test]
fn interface_errors_remain_editable_without_cancelling_the_point_prompt() {
    let mut app = test_app();
    for command in ["Line", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let plane = app.drafting_plane;
    let state = app.interface_state();
    for invalid in [
        "SetSnap",
        "Snap On",
        "DisableOsnap Yes",
        "SmartTrack Maybe",
        "SetDisplayMode Rendered",
        "Help Invalid",
    ] {
        enter(&mut app, invalid);
        assert_eq!(app.command_input, invalid);
        assert_eq!(app.active_command, pending);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.interface_state(), state);
        assert_eq!(app.document.objects().len(), 0);
        assert!(app.command_log.back().unwrap().contains("Usage:"));
    }
    enter(&mut app, "1,2");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn interface_actions_do_not_destroy_redo_history_or_partial_coordinate_text() {
    let mut app = test_app();
    for command in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, command);
    }
    app.command_input = "r1.5,".into();
    app.apply_interface_command(InterfaceCommand::SetSnap(SwitchAction::Toggle));
    assert_eq!(app.command_input, "r1.5,");
    assert!(app.active_command.is_some());
    assert!(app.document.can_redo());
    app.cancel_interactive_command(false);
    enter(&mut app, "Redo");
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn interface_names_are_discoverable_without_hiding_modeling_commands() {
    let mut app = test_app();
    for name in interface::COMMAND_NAMES {
        assert_eq!(
            command_completions(&app.commands, &format!("'_-{name}")),
            [name]
        );
    }
    enter(&mut app, "Help");
    let listing = app
        .command_log
        .iter()
        .find(|line| line.starts_with("Commands:"))
        .unwrap();
    for name in app
        .commands
        .command_names()
        .into_iter()
        .chain(interface::COMMAND_NAMES)
    {
        assert!(
            listing
                .split(|c: char| c.is_whitespace() || c == ',')
                .any(|word| word == name),
            "{name}"
        );
    }
}

fn key(key: egui::Key, modifiers: egui::Modifiers, pressed: bool, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat,
        modifiers,
    }
}

fn frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    width: f32,
    events: Vec<egui::Event>,
) -> (f32, egui::FullOutput) {
    let mut height = 0.0;
    let output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.handle_interface_shortcuts(ui);
            app.show_toolbar(ui);
            height = ui.available_rect_before_wrap().top();
            app.capture_global_command_typing(ui);
            app.show_command_line(ui);
        },
    );
    (height, output)
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

fn label_position(shapes: &[egui::epaint::ClippedShape], label: &str) -> egui::Pos2 {
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                // Wrapped horizontal labels include leading indentation in
                // their layout rect. Aim at the painted glyphs, not that rect.
                Some(text.pos + text.galley.mesh_bounds.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label))
        .unwrap_or_else(|| panic!("missing label {label}"))
}

fn click(context: &egui::Context, app: &mut VibocerosApp, position: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            context,
            app,
            1000.0,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .1
        .drop_without_applying_deltas();
    }
}

#[test]
fn interface_toolbar_is_compact_and_wraps_on_narrow_windows() {
    for (width, max_height) in [(1200.0, 40.0), (640.0, 65.0), (400.0, 90.0)] {
        let mut app = test_app();
        let context = egui::Context::default();
        frame(&context, &mut app, width, vec![])
            .1
            .drop_without_applying_deltas();
        let (height, output) = frame(&context, &mut app, width, vec![]);
        assert!(
            height > 0.0 && height <= max_height,
            "width={width}, height={height}"
        );
        for label in [
            "Undo",
            "Redo",
            "Grid Snap",
            "Osnap",
            "SmartTrack",
            "Millimetres",
            "Wireframe",
            "Top",
            "?",
        ] {
            let position = label_position(&output.shapes, label);
            assert!(
                position.x >= 0.0 && position.x < width && position.y < height,
                "{label}: {position:?}"
            );
        }
        output.drop_without_applying_deltas();
    }
}

#[test]
fn interface_toolbar_clicks_preserve_partial_input_and_disable_model_undo_during_drafting() {
    let mut app = test_app();
    let context = egui::Context::default();
    for command in ["Point 1,2,3", "Line", "0"] {
        enter(&mut app, command);
    }
    app.command_input = "r1,".into();
    let pending = app.active_command;
    for label in [
        "Undo",
        "Grid Snap",
        "Osnap",
        "SmartTrack",
        "Millimetres",
        "?",
    ] {
        frame(&context, &mut app, 1000.0, vec![])
            .1
            .drop_without_applying_deltas();
        let output = frame(&context, &mut app, 1000.0, vec![]).1;
        let position = label_position(&output.shapes, label);
        output.drop_without_applying_deltas();
        click(&context, &mut app, position);
        assert_eq!(app.command_input, "r1,");
        assert_eq!(app.active_command, pending);
        assert_eq!(app.document.objects().len(), 1);
    }
    assert!(!app.grid_snap && !app.osnap && !app.smart_track);
}

#[test]
fn interface_dropdowns_target_only_the_active_view_without_losing_a_latched_plane() {
    let mut app = test_app();
    let context = egui::Context::default();
    app.active_viewport = 3;
    for command in ["Circle", "0"] {
        enter(&mut app, command);
    }
    let pending = app.active_command;
    let plane = app.drafting_plane;
    app.command_input = "2,".into();
    for (menu, choice) in [("Right", "Front"), ("Wireframe", "Shaded")] {
        frame(&context, &mut app, 1000.0, vec![])
            .1
            .drop_without_applying_deltas();
        let output = frame(&context, &mut app, 1000.0, vec![]).1;
        let position = label_position(&output.shapes, menu);
        output.drop_without_applying_deltas();
        click(&context, &mut app, position);
        let output = frame(&context, &mut app, 1000.0, vec![]).1;
        let position = label_position(&output.shapes, choice);
        output.drop_without_applying_deltas();
        click(&context, &mut app, position);
        assert_eq!(app.active_command, pending);
        assert_eq!(app.drafting_plane, plane);
        assert_eq!(app.command_input, "2,");
    }
    assert_eq!(app.active_viewport, 3);
    assert_eq!(app.viewports[3].kind(), ViewKind::Front);
    assert_eq!(app.viewports[3].display_mode, DisplayMode::Shaded);
    assert_eq!(app.viewports[0].kind(), ViewKind::Top);
    assert!(
        app.viewports[..3]
            .iter()
            .all(|v| v.display_mode == DisplayMode::Wireframe)
    );
}

#[test]
fn interface_toolbar_undo_and_redo_edit_the_model_when_idle() {
    let mut app = test_app();
    let context = egui::Context::default();
    enter(&mut app, "Point 1,2,3");
    for (label, count) in [("Undo", 0), ("Redo", 1)] {
        frame(&context, &mut app, 1000.0, vec![])
            .1
            .drop_without_applying_deltas();
        let output = frame(&context, &mut app, 1000.0, vec![]).1;
        let position = label_position(&output.shapes, label);
        output.drop_without_applying_deltas();
        click(&context, &mut app, position);
        assert_eq!(app.document.objects().len(), count);
    }
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
