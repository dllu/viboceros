use super::*;

#[test]
fn snap_size_updates_scoped_viewports_and_prompts_without_model_edits() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "Undo");
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "SnapSize 0.25");
    assert_eq!(app.viewports[0].snap_spacing(), 0.25);
    assert!(
        app.viewports[1..]
            .iter()
            .all(|view| view.snap_spacing() == 1.0)
    );
    app.active_viewport = 2;
    enter(&mut app, "SnapSize ApplyTo=AllViewports");
    assert_eq!(
        app.snap_size_pending,
        Some((viboceros_command::interface::ViewportTarget::All, 2))
    );
    enter(&mut app, "0");
    assert!(app.snap_size_pending.is_some());
    assert_eq!(app.viewports[0].snap_spacing(), 0.25);
    enter(&mut app, "0.125");
    assert!(
        app.viewports
            .iter()
            .all(|view| view.snap_spacing() == 0.125)
    );
    assert!(app.snap_size_pending.is_none());
    app.active_viewport = 1;
    enter(&mut app, "SnapSize");
    app.active_viewport = 3;
    enter(&mut app, "0.5");
    assert_eq!(app.viewports[1].snap_spacing(), 0.5);
    assert_eq!(app.viewports[3].snap_spacing(), 0.125);
    enter(&mut app, "SnapSize");
    enter(&mut app, "");
    assert!(app.snap_size_pending.is_none());
    assert_eq!(app.viewports[3].snap_spacing(), 0.125);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().count(), 0);
}

#[test]
fn grid_command_updates_views_atomically_without_touching_model_history() {
    let mut app = test_app();
    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "Undo");
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let redo = app.document.redo_label().map(str::to_owned);
    enter(
        &mut app,
        "Grid MinorLineSpacing=0.5 MajorLineInterval=4 GridLineCount=30 ShowGrid=No ShowGridAxes=No ShowWorldAxes=Yes SnapSpacing=0.25",
    );
    let changed = app.viewports[0].grid_settings();
    assert_eq!(changed.minor_spacing, 0.5);
    assert_eq!(changed.snap_spacing, 0.25);
    assert_eq!(changed.major_interval, 4);
    assert_eq!(changed.line_count, 30);
    assert!(!changed.show_grid && !changed.show_axes && changed.show_world_axes);
    assert_eq!(app.viewports[1].grid_settings(), GridSettings::default());
    app.active_viewport = 2;
    enter(
        &mut app,
        "Grid MinorLineSpacing=2 ShowGrid=No ApplyTo=AllViewports",
    );
    assert!(
        app.viewports.iter().all(
            |view| view.grid_settings().minor_spacing == 2.0 && !view.grid_settings().show_grid
        )
    );
    let before = app
        .viewports
        .iter()
        .map(Viewport::grid_settings)
        .collect::<Vec<_>>();
    enter(
        &mut app,
        "Grid MinorLineSpacing=1e308 GridLineCount=100000 ApplyTo=AllViewports",
    );
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::grid_settings)
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Error: Grid settings exceed the supported finite range")
    );
    enter(&mut app, "Grid");
    assert!(app.grid_settings_open);
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    assert_eq!(app.document.objects().count(), 0);
}

#[test]
fn f7_toggles_active_grid_display_without_toggling_grid_snap() {
    let mut app = test_app();
    enter(&mut app, "Line");
    let pending = app.active_command;
    app.command_input = "1,".into();
    let context = egui::Context::default();
    for expected in [false, true] {
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events: vec![
                        key(egui::Key::F7, egui::Modifiers::NONE, true, false),
                        key(egui::Key::F7, egui::Modifiers::NONE, false, false),
                    ],
                    ..Default::default()
                },
                |ui| app.handle_interface_shortcuts(ui),
            )
            .drop_without_applying_deltas();
        assert_eq!(app.viewports[0].grid_settings().show_grid, expected);
        assert!(app.grid_snap);
        assert_eq!(app.active_command, pending);
        assert_eq!(app.command_input, "1,");
    }
    app.apply_interface_command(InterfaceCommand::ToggleGrid);
    app.apply_interface_command(InterfaceCommand::ToggleGrid);
    assert!(app.viewports[0].grid_settings().show_grid);
}

#[test]
fn ortho_commands_and_f8_preserve_the_active_point_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    enter(&mut app, "SetOrtho On");
    enter(&mut app, "OrthoSnapToCPlaneZ Enable");
    enter(&mut app, "OrthoAngle 45");
    assert!(app.ortho);
    assert!(app.ortho_snap_to_cplane_z);
    assert_eq!(app.ortho_angle.degrees(), 45.0);
    assert_eq!(app.active_command, pending);
    let context = egui::Context::default();
    for expected in [false, true] {
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events: vec![
                        key(egui::Key::F8, egui::Modifiers::NONE, true, false),
                        key(egui::Key::F8, egui::Modifiers::NONE, false, false),
                    ],
                    ..Default::default()
                },
                |ui| app.handle_interface_shortcuts(ui),
            )
            .drop_without_applying_deltas();
        assert_eq!(app.ortho, expected);
        assert_eq!(app.active_command, pending);
    }
}

#[test]
fn planar_commands_preserve_the_active_point_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    for (command, enabled) in [
        ("SetPlanar On", true),
        ("SetPlanar On", true),
        ("Planar", false),
        ("SetPlanar Toggle", true),
        ("SetPlanar Off", false),
    ] {
        enter(&mut app, command);
        assert_eq!(app.planar, enabled, "{command}");
        assert_eq!(app.active_command, pending);
    }
}
