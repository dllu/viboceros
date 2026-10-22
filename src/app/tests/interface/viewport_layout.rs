use super::*;

#[test]
fn viewport_tabs_command_controls_visibility_without_interrupting_modeling() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    assert!(app.viewport_tabs_visible);
    enter(&mut app, "ViewportTabs Hide");
    assert!(!app.viewport_tabs_visible);
    enter(&mut app, "ViewportTabs Show");
    assert!(app.viewport_tabs_visible);
    enter(&mut app, "ViewportTabs");
    assert!(!app.viewport_tabs_visible);
    enter(&mut app, "ViewportTabs bogus");
    assert!(!app.viewport_tabs_visible);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("Usage: ViewportTabs")
    );
    assert_eq!(app.active_command, pending);
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn viewport_tabs_align_command_places_all_four_edges_without_changing_modeling_state() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    for (command, expected) in [
        ("ViewportTabs Align=Top", ViewportTabAlignment::Top),
        ("_-ViewportTabs _Align=_Left", ViewportTabAlignment::Left),
        ("ViewportTabs Align Right", ViewportTabAlignment::Right),
        ("ViewportTabs Align=Bottom", ViewportTabAlignment::Bottom),
    ] {
        enter(&mut app, command);
        assert_eq!(app.viewport_tab_alignment, expected);
        assert!(app.viewport_tabs_visible);
        assert_eq!(app.active_command, pending);
    }
    enter(&mut app, "ViewportTabs Align=Diagonal");
    assert_eq!(app.viewport_tab_alignment, ViewportTabAlignment::Bottom);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("Usage: ViewportTabs")
    );
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn max_viewport_tracks_active_view_and_preserves_modeling_prompt() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    let cameras = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    enter(&mut app, "MaxViewport");
    assert_eq!(app.maximized_viewport, Some(0));
    enter(&mut app, "NextViewport");
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.maximized_viewport, Some(1));
    enter(&mut app, "4View");
    assert_eq!(app.maximized_viewport, None);
    assert_eq!(app.active_command, pending);
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::camera_snapshot)
            .collect::<Vec<_>>(),
        cameras
    );
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn three_view_layout_has_three_renderable_viewports_and_preserves_input() {
    let mut app = test_app();
    app.active_viewport = 1;
    enter(&mut app, "Grid SnapSpacing=0.25 MinorLineSpacing=2.5");
    let grid = app.viewports[1].grid_settings();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    enter(&mut app, "3View");
    assert_eq!(app.viewports.len(), 3);
    assert_eq!(
        app.viewports.iter().map(Viewport::kind).collect::<Vec<_>>(),
        vec![ViewKind::Top, ViewKind::Perspective, ViewKind::Front]
    );
    assert_eq!(app.viewport_positions, THREE_VIEWPORT_POSITIONS.to_vec());
    assert!(
        app.viewports
            .iter()
            .all(|viewport| viewport.grid_settings() == grid)
    );
    assert_eq!(app.active_viewport, 0);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "NextViewport");
    assert_eq!(app.active_viewport, 1);
    enter(&mut app, "MaxViewport");
    assert_eq!(app.maximized_viewport, Some(1));
    enter(&mut app, "MaxViewport");
    assert_eq!(app.maximized_viewport, None);
    assert_eq!(app.command_log.back().unwrap(), "Restored 3 viewports");
    enter(&mut app, "4View");
    assert_eq!(app.viewports.len(), 4);
    assert_eq!(app.viewport_positions, DEFAULT_VIEWPORT_POSITIONS.to_vec());
    assert_eq!(app.active_command, pending);
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn four_view_projection_options_restore_rhino_arrangements_and_active_perspective() {
    let mut app = test_app();
    app.active_viewport = 2;
    enter(&mut app, "Grid SnapSpacing=0.25 MinorLineSpacing=2.5");
    let grid = app.viewports[2].grid_settings();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    enter(&mut app, "4View Projection=FirstAngle");
    assert_eq!(
        app.viewports.iter().map(Viewport::kind).collect::<Vec<_>>(),
        [
            ViewKind::Front,
            ViewKind::Left,
            ViewKind::Top,
            ViewKind::Perspective
        ]
    );
    assert_eq!(app.viewport_positions, DEFAULT_VIEWPORT_POSITIONS);
    assert_eq!(app.active_viewport, 3);
    assert_eq!(app.viewports[0].grid_settings(), grid);
    assert!(
        app.viewports[1..]
            .iter()
            .all(|view| view.grid_settings() == GridSettings::default())
    );
    assert!(
        app.viewports
            .iter()
            .all(|view| view.display_mode == DisplayMode::Wireframe)
    );
    assert_eq!(app.active_command, pending);
    enter(&mut app, "4View");
    assert_eq!(app.viewports[1].kind(), ViewKind::Left);
    enter(&mut app, "_-4View _Projection _ThirdAngle");
    assert_eq!(
        app.viewports.iter().map(Viewport::kind).collect::<Vec<_>>(),
        [
            ViewKind::Top,
            ViewKind::Perspective,
            ViewKind::Front,
            ViewKind::Right
        ]
    );
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.viewport_positions, DEFAULT_VIEWPORT_POSITIONS);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "4View Projection=Invalid");
    assert_eq!(app.viewports[3].kind(), ViewKind::Right);
    assert!(app.command_log.back().unwrap().contains("Usage: 4View"));
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn plain_four_view_resets_changed_cameras_and_remembers_projection_choice() {
    let mut app = test_app();
    app.active_viewport = 0;
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    enter(&mut app, "SetView World Bottom");
    assert_eq!(app.viewports[0].kind(), ViewKind::Bottom);
    enter(&mut app, "4View");
    assert_eq!(
        app.viewports.iter().map(Viewport::kind).collect::<Vec<_>>(),
        [
            ViewKind::Top,
            ViewKind::Perspective,
            ViewKind::Front,
            ViewKind::Right
        ]
    );
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.active_command, pending);

    enter(&mut app, "4View Projection=FirstAngle");
    enter(&mut app, "3View");
    enter(&mut app, "SplitViewportVertical");
    assert_eq!(app.viewports.len(), 4);
    assert_ne!(app.viewport_positions, DEFAULT_VIEWPORT_POSITIONS);
    enter(&mut app, "4View");
    assert_eq!(
        app.viewports.iter().map(Viewport::kind).collect::<Vec<_>>(),
        [
            ViewKind::Front,
            ViewKind::Left,
            ViewKind::Top,
            ViewKind::Perspective
        ]
    );
    assert_eq!(app.viewport_positions, DEFAULT_VIEWPORT_POSITIONS);
    assert_eq!(app.active_viewport, 3);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn four_view_keeps_display_modes_only_for_views_that_survive_the_projection() {
    let mut app = test_app();
    app.viewports[1].display_mode = DisplayMode::Shaded;
    app.viewports[3].display_mode = DisplayMode::Ghosted;
    enter(&mut app, "4View");
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Shaded);
    assert_eq!(app.viewports[3].display_mode, DisplayMode::Ghosted);
    enter(&mut app, "4View Projection=FirstAngle");
    assert_eq!(app.viewports[3].display_mode, DisplayMode::Shaded);
    assert_eq!(app.viewports[1].kind(), ViewKind::Left);
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Wireframe);
    enter(&mut app, "4View Projection=ThirdAngle");
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Shaded);
    assert_eq!(app.viewports[3].display_mode, DisplayMode::Wireframe);
    app.viewports[0].display_mode = DisplayMode::Shaded;
    app.active_viewport = 0;
    enter(&mut app, "SetView World Bottom");
    enter(&mut app, "4View");
    assert_eq!(app.viewports[0].kind(), ViewKind::Top);
    assert_eq!(app.viewports[0].display_mode, DisplayMode::Wireframe);
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Shaded);
}

#[test]
fn four_view_keeps_grid_settings_with_surviving_views() {
    let mut app = test_app();
    app.active_viewport = 0;
    enter(&mut app, "Grid SnapSpacing=0.25 MinorLineSpacing=2.5");
    let top_grid = app.viewports[0].grid_settings();
    app.active_viewport = 3;
    enter(&mut app, "Grid SnapSpacing=0.75 MinorLineSpacing=7.5");
    let right_grid = app.viewports[3].grid_settings();

    enter(&mut app, "4View");
    assert_eq!(app.viewports[0].grid_settings(), top_grid);
    assert_eq!(app.viewports[3].grid_settings(), right_grid);
    assert_eq!(app.viewports[1].grid_settings(), GridSettings::default());
    enter(&mut app, "4View Projection=FirstAngle");
    assert_eq!(app.viewports[2].grid_settings(), top_grid);
    assert_eq!(app.viewports[1].grid_settings(), right_grid);
    assert_eq!(app.viewports[0].grid_settings(), GridSettings::default());
    enter(&mut app, "4View Projection=ThirdAngle");
    assert_eq!(app.viewports[0].grid_settings(), top_grid);
    assert_eq!(app.viewports[3].grid_settings(), GridSettings::default());
}

#[test]
fn four_view_reuses_changed_orthographic_grid_for_right() {
    for (index, command) in [(0, "SetView World Bottom"), (2, "SetView World Back")] {
        let mut app = test_app();
        app.active_viewport = index;
        enter(&mut app, "Grid SnapSpacing=0.25 MinorLineSpacing=2.5");
        let changed_grid = app.viewports[index].grid_settings();
        enter(&mut app, command);
        enter(&mut app, "4View");
        assert_eq!(app.viewports[index].grid_settings(), changed_grid);
        assert_eq!(app.viewports[3].grid_settings(), changed_grid);
        assert_eq!(app.viewports[1].grid_settings(), GridSettings::default());
    }
}

#[test]
fn split_viewport_commands_partition_the_active_view_and_keep_model_input() {
    let mut app = test_app();
    app.viewports[0].display_mode = DisplayMode::Ghosted;
    enter(&mut app, "Grid SnapSpacing=0.25 MinorLineSpacing=2.5");
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    let camera = app.viewports[0].camera_snapshot();
    let plane = app.viewports[0].construction_plane();
    let grid = app.viewports[0].grid_settings();
    enter(&mut app, "SplitViewportVertical");
    assert_eq!(app.viewports.len(), 5);
    assert_eq!(app.viewport_positions[0], [0.0, 0.25, 0.0, 0.5]);
    assert_eq!(app.viewport_positions[4], [0.25, 0.5, 0.0, 0.5]);
    assert_eq!(app.viewports[4].camera_snapshot(), camera);
    assert_eq!(app.viewports[4].construction_plane(), plane);
    assert_eq!(app.viewports[4].grid_settings(), grid);
    assert_eq!(app.viewports[4].display_mode, DisplayMode::Ghosted);
    assert_eq!(app.viewports[4].view_label(), "Top (2)");
    assert_eq!(app.active_command, pending);
    enter(&mut app, "SetActiveViewport Top (2)");
    assert_eq!(app.active_viewport, 4);
    enter(&mut app, "SplitViewportHorizontal");
    assert_eq!(app.viewports.len(), 6);
    assert_eq!(app.viewport_positions[4], [0.25, 0.5, 0.0, 0.25]);
    assert_eq!(app.viewport_positions[5], [0.25, 0.5, 0.25, 0.5]);
    assert_eq!(app.viewports[5].view_label(), "Top (3)");
    assert_eq!(app.active_viewport, 4);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn split_viewport_rejects_an_unrepresentable_midpoint_atomically() {
    let mut app = test_app();
    app.viewport_positions[0] = [0.0, f64::from_bits(1), 0.0, 0.5];
    let positions = app.viewport_positions.clone();
    enter(&mut app, "SplitViewportVertical");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(app.viewports.len(), 4);
    assert_eq!(app.viewport_positions, positions);
    assert_eq!(app.active_viewport, 0);
}

#[test]
fn new_viewport_overlays_a_top_view_and_close_restores_its_parent() {
    let mut app = test_app();
    app.active_viewport = 1;
    app.viewports[1].display_mode = DisplayMode::Ghosted;
    enter(&mut app, "Grid SnapSpacing=0.25 MinorLineSpacing=2.5");
    let grid = app.viewports[1].grid_settings();
    let original_positions = app.viewport_positions.clone();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    enter(&mut app, "NewViewport");
    assert_eq!(app.viewports.len(), 5);
    assert_eq!(app.viewport_positions[4], [0.25, 0.75, 0.25, 0.75]);
    assert_eq!(app.active_viewport, 4);
    assert_eq!(app.viewports[4].kind(), ViewKind::Top);
    assert_eq!(app.viewports[4].view_label(), "Top");
    assert_eq!(app.viewports[4].display_mode, DisplayMode::Wireframe);
    assert_eq!(app.viewports[4].grid_settings(), grid);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 4);
    assert_eq!(app.viewport_positions, original_positions);
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn repeated_new_viewports_stack_at_the_same_position_and_close_in_order() {
    let mut app = test_app();
    app.active_viewport = 1;
    enter(&mut app, "NewViewport");
    enter(&mut app, "NewViewport");
    assert_eq!(app.viewports.len(), 6);
    assert_eq!(app.active_viewport, 5);
    assert_eq!(app.viewport_positions[4], app.viewport_positions[5]);
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 5);
    assert_eq!(app.active_viewport, 4);
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 4);
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.viewport_positions, DEFAULT_VIEWPORT_POSITIONS.to_vec());
}

#[test]
fn close_half_of_split_overlapping_viewport_expands_its_sibling() {
    let mut app = test_app();
    app.active_viewport = 1;
    enter(&mut app, "NewViewport");
    enter(&mut app, "SplitViewportVertical");
    assert_eq!(app.viewport_positions[4], [0.25, 0.5, 0.25, 0.75]);
    assert_eq!(app.viewport_positions[5], [0.5, 0.75, 0.25, 0.75]);
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 5);
    assert_eq!(app.viewport_positions[4], [0.25, 0.75, 0.25, 0.75]);
    assert_eq!(app.viewport_positions[..4], DEFAULT_VIEWPORT_POSITIONS);
}

#[test]
fn close_tiled_view_beneath_new_viewport_keeps_overlay_and_rebases_parent() {
    let mut app = test_app();
    app.active_viewport = 1;
    enter(&mut app, "NewViewport");
    app.active_viewport = 0;
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 4);
    assert_eq!(app.viewport_positions[0], [0.0, 1.0, 0.0, 0.5]);
    assert_eq!(app.viewport_positions[3], [0.25, 0.75, 0.25, 0.75]);
    assert_eq!(app.viewports[3].new_viewport_parent, Some(0));
    app.active_viewport = 3;
    enter(&mut app, "CloseViewport");
    assert_eq!(app.active_viewport, 0);
    assert_eq!(app.viewports.len(), 3);
    assert_eq!(app.viewport_positions[0], [0.0, 1.0, 0.0, 0.5]);
}

#[test]
fn close_viewport_fills_a_split_strip_and_keeps_the_model_prompt() {
    let mut app = test_app();
    enter(&mut app, "SplitViewportVertical");
    enter(&mut app, "SetActiveViewport Top (2)");
    enter(&mut app, "SplitViewportHorizontal");
    enter(&mut app, "SetActiveViewport Top");
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 5);
    assert_eq!(app.viewport_positions[3], [0.0, 0.5, 0.0, 0.25]);
    assert_eq!(app.viewport_positions[4], [0.0, 0.5, 0.25, 0.5]);
    assert_eq!(app.active_command, pending);
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn close_viewport_remaps_pending_view_references_and_keeps_one_view() {
    let mut app = test_app();
    app.zoom_factor_pending = Some(3);
    app.snap_size_pending = Some((interface::ViewportTarget::Active, 2));
    app.zoom_target = Some(ZoomTargetState::PickWindow {
        target: Point3::try_new(0.0, 0.0, 0.0).unwrap(),
        viewport: 3,
    });
    app.circular_selection = Some(CircularSelectionState::PickRadius {
        mode: interface::RectSelectionMode::Window,
        center: egui::Pos2::ZERO,
        viewport: 2,
    });
    app.active_viewport = 1;
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 3);
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.viewport_positions[0], [0.0, 1.0, 0.0, 0.5]);
    assert_eq!(app.zoom_factor_pending, Some(2));
    assert_eq!(
        app.snap_size_pending,
        Some((interface::ViewportTarget::Active, 1))
    );
    assert!(matches!(
        app.zoom_target,
        Some(ZoomTargetState::PickWindow { viewport: 2, .. })
    ));
    assert!(matches!(
        app.circular_selection,
        Some(CircularSelectionState::PickRadius { viewport: 1, .. })
    ));
    app.active_viewport = 2;
    enter(&mut app, "CloseViewport");
    assert_eq!(app.zoom_factor_pending, None);
    assert!(app.zoom_target.is_none());
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 1);
    assert_eq!(app.viewport_positions, vec![[0.0, 1.0, 0.0, 1.0]]);
    assert!(app.snap_size_pending.is_none());
    assert!(app.circular_selection.is_none());
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 1);
    assert_eq!(
        app.command_log.back().unwrap(),
        "Error: Cannot close the last viewport"
    );
}

#[test]
fn close_maximized_viewport_restores_the_remaining_layout() {
    let mut app = test_app();
    app.active_viewport = 3;
    enter(&mut app, "MaxViewport");
    assert_eq!(app.maximized_viewport, Some(3));
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewports.len(), 3);
    assert_eq!(app.active_viewport, 2);
    assert_eq!(app.maximized_viewport, None);
    assert_eq!(app.viewport_positions[2], [0.0, 1.0, 0.5, 1.0]);
}

#[test]
fn close_viewport_retiles_when_saved_positions_have_no_adjacent_strip() {
    let mut app = test_app();
    app.viewport_positions = vec![
        [0.0, 0.2, 0.0, 0.2],
        [0.3, 0.5, 0.0, 0.2],
        [0.0, 0.2, 0.3, 0.5],
        [0.3, 0.5, 0.3, 0.5],
    ];
    enter(&mut app, "CloseViewport");
    assert_eq!(app.viewport_positions, THREE_VIEWPORT_POSITIONS.to_vec());
}

#[test]
fn viewport_properties_title_names_the_active_view_without_losing_model_input() {
    let mut app = test_app();
    app.active_viewport = 2;
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    enter(&mut app, "-_ViewportProperties _Title=\"South Elevation\"");
    assert_eq!(app.viewports[2].view_label(), "South Elevation");
    assert_eq!(app.active_command, pending);
    app.active_viewport = 0;
    enter(&mut app, "SetActiveViewport South Elevation");
    assert_eq!(app.active_viewport, 2);
    enter(&mut app, "-ViewportProperties Title=\"\"");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(app.viewports[2].view_label(), "South Elevation");
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn named_viewport_commands_select_existing_titles_and_preserve_modeling_input() {
    let mut app = test_app();
    enter(&mut app, "Line");
    enter(&mut app, "0,0,0");
    let pending = app.active_command;
    let cameras = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    enter(&mut app, "SetActiveViewport front");
    assert_eq!(app.active_viewport, 2);
    assert_eq!(app.maximized_viewport, None);
    enter(&mut app, "SetMaximizedViewport Right");
    assert_eq!(app.active_viewport, 3);
    assert_eq!(app.maximized_viewport, Some(3));
    enter(&mut app, "SetMaximizedViewport Right");
    assert_eq!(app.maximized_viewport, Some(3));
    enter(&mut app, "_SetActiveViewport Perspective");
    assert_eq!(app.active_viewport, 1);
    assert_eq!(app.maximized_viewport, Some(1));
    for input in ["SetActiveViewport Missing", "SetMaximizedViewport 0"] {
        enter(&mut app, input);
        assert!(app.command_log.back().unwrap().starts_with("Error:"));
        assert_eq!(app.active_viewport, 1);
        assert_eq!(app.maximized_viewport, Some(1));
    }
    enter(&mut app, "SetActiveViewport 1");
    assert_eq!(app.active_viewport, 0);
    assert_eq!(app.maximized_viewport, Some(0));
    assert_eq!(app.active_command, pending);
    assert_eq!(
        app.viewports
            .iter()
            .map(Viewport::camera_snapshot)
            .collect::<Vec<_>>(),
        cameras
    );
    enter(&mut app, "1,0,0");
    assert_eq!(app.document.objects().count(), 1);
}

#[test]
fn named_viewport_commands_reject_ambiguous_titles_without_switching() {
    let mut app = test_app();
    app.active_viewport = 1;
    enter(&mut app, "SetView World Top");
    app.active_viewport = 3;
    enter(&mut app, "SetActiveViewport Top");
    assert!(app.command_log.back().unwrap().contains("ambiguous"));
    assert_eq!(app.active_viewport, 3);
    enter(&mut app, "SetActiveViewport 2");
    assert_eq!(app.active_viewport, 1);
    enter(&mut app, "SetView CPlane Front");
    app.active_viewport = 3;
    enter(&mut app, "SetActiveViewport \"CPlane Front\"");
    assert_eq!(app.active_viewport, 1);
}

#[test]
fn double_clicking_viewport_title_requests_layout_toggle() {
    let context = egui::Context::default();
    let mut viewport = Viewport::new(ViewKind::Top);
    let document = Document::default();
    let mut click = |position: egui::Pos2| {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events: [true, false]
                        .map(|pressed| egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        })
                        .to_vec(),
                    ..Default::default()
                },
                |ui| {
                    output = viewport.show(ui, &document, ViewportInput::default(), &[], 0, true);
                },
            )
            .drop_without_applying_deltas();
        output
    };
    assert!(!click(egui::pos2(30.0, 12.0)).toggle_maximized);
    assert!(click(egui::pos2(30.0, 12.0)).toggle_maximized);
}

#[test]
fn normalized_viewport_position_sets_the_rendered_camera_port() {
    let context = egui::Context::default();
    let mut viewport = Viewport::new(ViewKind::Top);
    let document = Document::default();
    let mut expected = [0, 0];
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                let rect = viewport_rect(ui.available_rect_before_wrap(), [0.0, 0.3, 0.0, 0.7]);
                expected = [rect.width() as i32, rect.height() as i32];
                let mut child = ui.new_child(egui::UiBuilder::new().id_salt(0).max_rect(rect));
                child.set_clip_rect(rect);
                viewport.show(
                    &mut child,
                    &document,
                    ViewportInput::default(),
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    let camera = Viewport::named_view_to_3dm(viewport.named_view_snapshot(), "Top".into()).unwrap();
    assert_eq!(camera.screen_port, [0, expected[0], expected[1], 0]);
    assert!(expected[0] < 400 && expected[1] < 500);
}
