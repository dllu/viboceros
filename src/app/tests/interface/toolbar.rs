use super::*;

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
