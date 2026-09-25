use super::*;
use viboceros_command::interface::{self, InterfaceCommand, SwitchAction, ViewportTabAlignment};

fn enter(app: &mut VibocerosApp, command: &str) {
    app.command_input = command.into();
    app.run_command();
}

#[test]
fn circle_size_option_works_through_the_command_line() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "0,0,0");
    enter(&mut app, "Diameter");
    enter(&mut app, "6");
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().next().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 3.0).abs() < 1e-12
    ));
}

#[test]
fn vertical_circle_uses_a_direction_pick_after_numeric_radius() {
    let mut app = test_app();
    enter(&mut app, "Circle");
    enter(&mut app, "Vertical");
    enter(&mut app, "1,2,3");
    enter(&mut app, "4");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleVertical {
            center: Some(_),
            radius: Some(4.0),
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(9.0, 2.0, 3.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
            && circle.point_at_angle(0.0).unwrap() == point(5.0, 2.0, 3.0)
    ));

    enter(&mut app, "Circle Vertical");
    assert!(app.accept_drafting_point(point(1.0, 2.0, 3.0)));
    assert!(!app.accept_drafting_point(point(1.0, 2.0, 7.0)));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::CircleVertical {
            center: Some(_),
            ..
        })
    ));
    assert!(app.accept_drafting_point(point(5.0, 2.0, 5.0)));
    assert_eq!(app.active_command, None);
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if (circle.radius() - 20.0_f64.sqrt()).abs() < 1e-12
    ));

    enter(&mut app, "Circle Vertical");
    assert!(app.accept_drafting_point(point(1.0, 2.0, 3.0)));
    enter(&mut app, "Diameter 8");
    assert!(app.accept_drafting_point(point(9.0, 2.0, 3.0)));
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Circle(circle) if circle.radius() == 4.0
    ));
}

fn layout_viewports(context: &egui::Context, app: &mut VibocerosApp) {
    for index in 0..app.viewports.len() {
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
                    app.viewports[index].show(
                        ui,
                        &app.document,
                        ViewportInput::default(),
                        &[],
                        index,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
    }
}

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

fn zoom_window_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        zoom_window: true,
                        drafting: DraftingInput {
                            active: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    output
}

fn zoom_target_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    mode: ZoomTargetInput,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        zoom_target: Some(mode),
                        drafting: DraftingInput {
                            active: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    output
}

fn selection_capture_frame(
    context: &egui::Context,
    app: &mut VibocerosApp,
    events: Vec<egui::Event>,
) -> ViewportOutput {
    let mut output = ViewportOutput::default();
    let object_filter = if app
        .fence_selection
        .as_ref()
        .is_some_and(|state| state.curve_pick)
        || app.boundary_selection.is_some()
    {
        Some(viboceros_command::ObjectSelectionFilter::Curves)
    } else {
        app.viewport_object_filter()
    };
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
                output = app.viewports[0].show(
                    ui,
                    &app.document,
                    ViewportInput {
                        object_filter,
                        rect_selection_mode: app.selection_window_override,
                        circular_selection: match app.circular_selection {
                            Some(CircularSelectionState::PickCenter(_)) => {
                                Some(CircularSelectionInput::PickCenter)
                            }
                            Some(CircularSelectionState::PickRadius {
                                mode,
                                center,
                                viewport: 0,
                            }) => Some(CircularSelectionInput::PickRadius { mode, center }),
                            Some(CircularSelectionState::PickRadius { .. }) => {
                                Some(CircularSelectionInput::Waiting)
                            }
                            None => None,
                        },
                        fence_selection: match app.fence_selection.as_ref() {
                            Some(state) if state.curve_pick => None,
                            Some(state) if state.viewport.is_none() => {
                                Some(FenceSelectionInput::PickFirst)
                            }
                            Some(state) if state.viewport == Some(0) => {
                                Some(FenceSelectionInput::Continue(&state.points))
                            }
                            Some(_) => Some(FenceSelectionInput::Waiting),
                            None => None,
                        },
                        lasso_selection: match app.lasso_selection.as_ref() {
                            Some(state)
                                if state.viewport.is_none() || state.viewport == Some(0) =>
                            {
                                Some(LassoSelectionInput::Capture {
                                    points: &state.points,
                                    mode: state.mode,
                                })
                            }
                            Some(_) => Some(LassoSelectionInput::Waiting),
                            None => None,
                        },
                        ..Default::default()
                    },
                    &[],
                    0,
                    true,
                );
            },
        )
        .drop_without_applying_deltas();
    output
}

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
fn pipe_picks_rail_and_radius_or_uses_preselected_rail() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 10,0,0");
    let source = app.document.objects().next().unwrap().id();
    enter(&mut app, "Pipe Cap=None");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Pipe { source: None, .. })
    ));
    app.apply_selection_click(SelectionClick {
        object_id: Some(source),
        mode: SelectionMode::Replace,
    });
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Pipe {
            source: Some(id),
            cap_flat: false,
            ..
        }) if id == source
    ));
    assert!(app.accept_drafting_point(Point3::try_new(5., 1., 0.).unwrap()));
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::NurbsSurface(_)
    ));

    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pipe 0.5 Cap=Flat");
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed()
    ));

    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pipe Cap=Flat Thick=Yes WallThickness=0.2");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Pipe {
            source: Some(id),
            wall_thickness: Some(thickness),
            ..
        }) if id == source && thickness == 0.2
    ));
    assert!(app.accept_drafting_point(Point3::try_new(5., 1., 0.).unwrap()));
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed() && brep.faces().len() == 4
    ));

    app.document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "Pipe Thick=Yes");
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Pipe {
            source: Some(id),
            pick_second_radius: true,
            first_radius: None,
            ..
        }) if id == source
    ));
    assert!(app.accept_drafting_point(Point3::try_new(5., 1., 0.).unwrap()));
    assert!(matches!(
        app.active_command,
        Some(InteractiveCommand::Pipe {
            first_radius: Some(radius),
            ..
        }) if radius == 1.0
    ));
    assert!(app.accept_drafting_point(Point3::try_new(5., 1.2, 0.).unwrap()));
    assert!(app.active_command.is_none());
    assert!(matches!(
        app.document.objects().last().unwrap().geometry(),
        Geometry::Brep(brep) if brep.is_closed() && brep.faces().len() == 4
    ));
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
    let initial = app.viewports[0].camera_snapshot();
    zoom_target_frame(&context, &mut app, ZoomTargetInput::PickTarget, vec![]);
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
fn zoom_ends_preserves_modeling_input_selection_and_document_history() {
    let mut app = test_app();
    for command in [
        "Polyline 0,0,0 1000,500,0 2,0,0",
        "SelAll",
        "Point 5,5,5",
        "Undo",
        "Line",
        "0",
    ] {
        enter(&mut app, command);
    }
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let before = app
        .viewports
        .iter()
        .map(Viewport::camera_snapshot)
        .collect::<Vec<_>>();
    let pending = app.active_command;
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let redo = app.document.redo_label().map(str::to_owned);
    enter(&mut app, "ZoomEnds All");
    assert_ne!(app.viewports[0].camera_snapshot(), before[0]);
    for (index, snapshot) in before.iter().enumerate().skip(1) {
        assert_eq!(app.viewports[index].camera_snapshot(), *snapshot);
    }
    assert_eq!(app.active_command, pending);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.redo_label(), redo.as_deref());
    enter(&mut app, "UndoView");
    assert_eq!(app.viewports[0].camera_snapshot(), before[0]);
    app.document.clear_selection();
    enter(&mut app, "ZoomEnds");
    assert_eq!(app.viewports[0].camera_snapshot(), before[0]);
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("No visible curve end markers to zoom to")
    );
}

#[test]
fn show_ends_filters_live_markers_and_zoom_uses_session_sources() {
    let mut app = test_app();
    for command in ["Polyline 0,0,0 10,10,0 2,0,0", "SelAll", "Line", "0"] {
        enter(&mut app, command);
    }
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let pending = app.active_command;
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    enter(&mut app, "ShowEnds");
    let analysis = app.end_analysis.as_ref().unwrap();
    assert_eq!(analysis.sources, selected);
    let markers = collect_end_markers(
        &app.document,
        analysis.sources.iter().copied(),
        analysis.options,
    )
    .unwrap();
    assert_eq!(markers.len(), 2);
    assert_eq!(app.active_command, pending);
    app.document.clear_selection();
    app.end_analysis.as_mut().unwrap().options.ends = false;
    let single_marker = collect_end_markers(
        &app.document,
        selected.iter().copied(),
        app.end_analysis.as_ref().unwrap().options,
    )
    .unwrap();
    assert_eq!(single_marker.len(), 1);
    enter(&mut app, "ZoomEnds");
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(0., 0., 0.)
    );
    app.end_analysis.as_mut().unwrap().options.ends = true;
    enter(&mut app, "ZoomEnds");
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(1., 0., 0.)
    );
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    enter(&mut app, "ShowEndsOff");
    assert!(app.end_analysis.is_none());
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "ZoomEnds");
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
}

#[test]
fn zoom_ends_cycles_session_markers_and_preserves_position_on_failure() {
    let mut app = test_app();
    enter(&mut app, "Polyline 0,0,0 10,10,0 2,0,0");
    enter(&mut app, "SelAll");
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(
        app.command_log.back().map(String::as_str),
        Some("Run ShowEnds with visible selected curves first")
    );
    enter(&mut app, "ShowEnds");
    enter(&mut app, "ZoomEnds Current");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 0);
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(0., 0., 0.)
    );
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 1);
    assert_eq!(
        app.viewports[0].camera_snapshot().target,
        nalgebra::Vector3::new(2., 0., 0.)
    );
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 0);
    enter(&mut app, "ZoomEnds Previous");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 1);
    app.end_analysis.as_mut().unwrap().options = EndMarkerOptions {
        starts: false,
        ends: false,
        seams: false,
        joints: false,
    };
    let camera = app.viewports[0].camera_snapshot();
    enter(&mut app, "ZoomEnds Next");
    assert_eq!(app.end_analysis.as_ref().unwrap().current, 1);
    assert_eq!(app.viewports[0].camera_snapshot(), camera);
}

#[test]
fn zoom_ends_mark_creates_one_undo_step_for_current_or_all_markers() {
    let mut app = test_app();
    enter(&mut app, "Polyline 0,0,0 10,10,0 2,0,0");
    enter(&mut app, "SelAll");
    let context = egui::Context::default();
    layout_viewports(&context, &mut app);
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    let original_count = app.document.objects().count();
    enter(&mut app, "ShowEnds");
    enter(&mut app, "ZoomEnds Mark");
    assert_eq!(app.document.objects().count(), original_count + 1);
    assert_eq!(app.document.undo_label(), Some("Mark curve ends"));
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), original_count);
    enter(&mut app, "ZoomEnds All");
    enter(&mut app, "ZoomEnds Mark");
    assert_eq!(app.document.objects().count(), original_count + 2);
    assert_eq!(app.document.undo_label(), Some("Mark curve ends"));
    enter(&mut app, "Undo");
    assert_eq!(app.document.objects().count(), original_count);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
}

#[test]
fn end_analysis_adds_and_removes_selected_curves_without_model_edits() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 1,0,0");
    enter(&mut app, "Line 10,0,0 11,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    enter(&mut app, "ShowEnds");
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[0]]);
    app.document
        .select_object(ids[1], SelectionMode::Replace)
        .unwrap();
    let before_objects = app.document.objects().cloned().collect::<Vec<_>>();
    let before_history = app.document.undo_label().map(str::to_owned);
    let selected = app.document.selected_object_ids().collect::<Vec<_>>();
    app.add_selected_to_end_analysis();
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        selected
    );
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, ids);
    assert_eq!(
        collect_end_markers(
            &app.document,
            ids.iter().copied(),
            EndMarkerOptions::default()
        )
        .unwrap()
        .len(),
        4
    );
    app.add_selected_to_end_analysis();
    assert_eq!(app.end_analysis.as_ref().unwrap().sources.len(), 2);
    app.document
        .select_object(ids[0], SelectionMode::Replace)
        .unwrap();
    app.remove_selected_from_end_analysis();
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[1]]);
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[0]]
    );
    app.document
        .select_object(ids[1], SelectionMode::Replace)
        .unwrap();
    app.remove_selected_from_end_analysis();
    assert!(app.end_analysis.as_ref().unwrap().sources.is_empty());
    assert_eq!(
        app.document.objects().cloned().collect::<Vec<_>>(),
        before_objects
    );
    assert_eq!(app.document.undo_label(), before_history.as_deref());
    assert_eq!(
        app.document.selected_object_ids().collect::<Vec<_>>(),
        [ids[1]]
    );
}

#[test]
fn show_ends_postselection_picks_curves_without_interrupting_modeling() {
    let mut app = test_app();
    enter(&mut app, "Line 0,0,0 1,0,0");
    enter(&mut app, "Line 10,0,0 11,0,0");
    let ids = app
        .document
        .objects()
        .map(|object| object.id())
        .collect::<Vec<_>>();
    app.document.clear_selection();
    enter(&mut app, "Line");
    enter(&mut app, "0");
    let pending = app.active_command;
    let objects = app.document.objects().cloned().collect::<Vec<_>>();
    let undo = app.document.undo_label().map(str::to_owned);
    app.zoom_factor_pending = Some(0);
    enter(&mut app, "ShowEnds");
    assert_eq!(
        app.end_analysis_pick.as_ref().unwrap().mode,
        EndAnalysisPickMode::Show
    );
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    app.apply_selection_region(
        SelectionWindow {
            object_ids: vec![ids[1]],
            mode: SelectionMode::Replace,
            crossing: false,
            inverted: false,
        },
        false,
    );
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, ids);
    assert_eq!(app.document.selected_object_count(), 0);
    enter(&mut app, "");
    assert!(app.end_analysis_pick.is_none());
    assert_eq!(app.zoom_factor_pending, Some(0));
    assert_eq!(app.active_command, pending);
    assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(app.document.undo_label(), undo.as_deref());
    app.start_end_analysis_pick(EndAnalysisPickMode::Remove);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[1]]);
    app.cancel_end_analysis_pick(true);
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, ids);
    app.start_end_analysis_pick(EndAnalysisPickMode::Remove);
    app.apply_selection_click(SelectionClick {
        object_id: Some(ids[0]),
        mode: SelectionMode::Replace,
    });
    enter(&mut app, "");
    assert_eq!(app.end_analysis.as_ref().unwrap().sources, [ids[1]]);
    assert_eq!(app.document.selected_object_count(), 0);
}

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
fn set_view_world_resets_each_standard_camera_and_keeps_model_history() {
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
    let untouched = app.viewports[0].camera_snapshot();
    for (name, kind, plane) in [
        ("Top", ViewKind::Top, Some(WorldPlane::Top)),
        ("Bottom", ViewKind::Bottom, Some(WorldPlane::Bottom)),
        ("Front", ViewKind::Front, Some(WorldPlane::Front)),
        ("Back", ViewKind::Back, Some(WorldPlane::Back)),
        ("Right", ViewKind::Right, Some(WorldPlane::Right)),
        ("Left", ViewKind::Left, Some(WorldPlane::Left)),
        ("Perspective", ViewKind::Perspective, None),
    ] {
        enter(&mut app, "Zoom Factor 2");
        let before = app.viewports[2].camera_snapshot();
        let plane_before = app.viewports[2].construction_plane();
        enter(&mut app, &format!("SetView World {name}"));
        assert_eq!(app.viewports[2].kind(), kind);
        assert_eq!(
            app.viewports[2].camera_snapshot(),
            Viewport::new(kind).camera_snapshot()
        );
        assert_eq!(
            app.viewports[2].construction_plane(),
            plane.map_or(plane_before, WorldPlane::frame)
        );
        enter(&mut app, "UndoView");
        assert_eq!(app.viewports[2].camera_snapshot(), before);
        enter(&mut app, "RedoView");
        assert_eq!(
            app.viewports[2].camera_snapshot(),
            Viewport::new(kind).camera_snapshot()
        );
        assert_eq!(app.viewports[0].camera_snapshot(), untouched);
        assert_eq!(app.active_command, pending);
        assert_eq!(app.document.redo_label(), redo.as_deref());
    }
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
        "SnapToMeshes Enable",
        "SnapToMeshes Toggle",
        "SmartTrack Off",
        "SetPlanar On",
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
    assert!(app.grid_snap && app.planar && !app.osnap && !app.smart_track);
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
        "SnapToMeshes On",
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
        let expected = match name {
            "ZE" => vec!["ZE", "ZEA"],
            "ZS" => vec!["ZS", "ZSA"],
            _ => vec![name],
        };
        assert_eq!(
            command_completions(&app.commands, &format!("'_-{name}"))[..expected.len()],
            expected
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
            "Ortho",
            "Planar",
            "Osnap",
            "Snap modes",
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
fn snap_menu_real_clicks_toggle_isolate_and_apply_a_one_shot_without_losing_input() {
    use viboceros_drafting::{ObjectSnapKind, ObjectSnapModes};
    let mut app = test_app();
    let context = egui::Context::default();
    enter(&mut app, "Line");
    app.command_input = "r1,".into();
    let pending = app.active_command;
    for (label, button, modifiers) in [
        (
            "Snap modes",
            egui::PointerButton::Primary,
            egui::Modifiers::NONE,
        ),
        ("Mid", egui::PointerButton::Primary, egui::Modifiers::NONE),
        ("Cen", egui::PointerButton::Secondary, egui::Modifiers::NONE),
        ("Cen", egui::PointerButton::Secondary, egui::Modifiers::NONE),
        (
            "Snap to mesh wires",
            egui::PointerButton::Primary,
            egui::Modifiers::NONE,
        ),
        ("Near", egui::PointerButton::Primary, egui::Modifiers::SHIFT),
    ] {
        frame(&context, &mut app, 1000., vec![])
            .1
            .drop_without_applying_deltas();
        let output = frame(&context, &mut app, 1000., vec![]).1;
        let position = label_position(&output.shapes, label);
        output.drop_without_applying_deltas();
        for pressed in [true, false] {
            frame(
                &context,
                &mut app,
                1000.,
                vec![
                    egui::Event::ModifiersChanged(modifiers),
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button,
                        pressed,
                        modifiers,
                    },
                ],
            )
            .1
            .drop_without_applying_deltas();
        }
        assert_eq!(app.command_input, "r1,");
        assert_eq!(app.active_command, pending);
        assert!(app.document.objects().next().is_none());
    }
    assert_eq!(app.one_shot_snap_label(), Some("Near"));
    assert!(app.snaps.mesh_edges);
    assert_eq!(
        app.effective_snap_modes(),
        ObjectSnapModes::only(ObjectSnapKind::Near)
    );
    assert_eq!(
        app.snaps.persistent,
        ObjectSnapModes::LANDMARKS.with(ObjectSnapKind::Mid, false)
    );
    app.accept_drafting_point(point(1., 2., 3.));
    assert_eq!(app.one_shot_snap_label(), None);
    assert_eq!(app.effective_snap_modes(), app.snaps.persistent);
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
        "Ortho",
        "Planar",
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
    assert!(!app.grid_snap && app.ortho && app.planar && !app.osnap && !app.smart_track);
    let output = frame(&context, &mut app, 1000.0, vec![]).1;
    let position = label_position(&output.shapes, "Ortho");
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        frame(
            &context,
            &mut app,
            1000.0,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .1
        .drop_without_applying_deltas();
    }
    assert!(app.ortho && app.ortho_snap_to_cplane_z);
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
fn edge_and_group_prompts_disable_model_history_buttons() {
    for command in ["SplitEdge", "AddToGroup"] {
        let mut app = test_app();
        let context = egui::Context::default();
        for command in ["Point 1,2,3", "Point 4,5,6", "Undo", "SelNone", command] {
            enter(&mut app, command);
        }
        assert!(app.document.can_undo() && app.document.can_redo());
        assert!(app.edge_prompt.is_some() || app.group_prompt.is_some());
        let objects = app.document.objects().cloned().collect::<Vec<_>>();
        for label in ["Undo", "Redo"] {
            frame(&context, &mut app, 1000., vec![])
                .1
                .drop_without_applying_deltas();
            let output = frame(&context, &mut app, 1000., vec![]).1;
            let position = label_position(&output.shapes, label);
            output.drop_without_applying_deltas();
            click(&context, &mut app, position);
            assert_eq!(app.document.objects().cloned().collect::<Vec<_>>(), objects);
            assert!(app.edge_prompt.is_some() || app.group_prompt.is_some());
            assert!(app.document.can_undo() && app.document.can_redo());
        }
    }
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
