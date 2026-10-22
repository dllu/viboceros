use super::*;

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
