use super::*;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn named_view_restores_camera_projection_and_cplane_in_another_viewport() {
    let mut app = test_app();
    enter(&mut app, "SetView World Perspective");
    enter(&mut app, "CPlane World Left");
    enter(&mut app, "SetView CPlane Front");
    let saved = app.viewports[0].named_view_snapshot();
    enter(&mut app, "NamedView Save Upper left");
    assert_eq!(
        app.named_views.names().collect::<Vec<_>>(),
        vec!["Upper left"]
    );
    let other_before = app.viewports[1].camera_snapshot();
    app.active_viewport = 1;
    app.viewports[1].display_mode = DisplayMode::Ghosted;
    enter(&mut app, "NamedView Restore upper LEFT");
    assert_eq!(app.viewports[1].named_view_snapshot(), saved);
    assert_eq!(app.viewports[1].view_label(), "Upper left");
    assert!(!app.viewports[1].title_modified());
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Ghosted);
    assert_eq!(app.viewports[0].named_view_snapshot(), saved);
    assert_eq!(app.document.undo_label(), None);
    assert!(app.viewports[1].undo_view());
    assert_eq!(app.viewports[1].camera_snapshot(), other_before);
    assert!(app.viewports[1].title_modified());
}

#[test]
fn named_view_edits_preserve_snapshot_and_reject_duplicate_names() {
    let mut app = test_app();
    enter(&mut app, "NamedView Save Front detail");
    let first = *app.named_views.get("front detail").unwrap();
    enter(&mut app, "NamedView Save FRONT DETAIL");
    assert_eq!(app.named_views.names().count(), 1);
    assert!(app.command_log.back().unwrap().contains("already exists"));
    enter(&mut app, "NamedView Duplicate Front detail | Copy");
    enter(&mut app, "NamedView Rename Copy | Work view");
    assert_eq!(*app.named_views.get("work view").unwrap(), first);
    enter(&mut app, "NamedView MoveUp Work view");
    assert_eq!(
        app.named_views.names().collect::<Vec<_>>(),
        vec!["Work view", "Front detail"]
    );
    enter(&mut app, "SetView World Right");
    enter(&mut app, "NamedView Update Work view");
    assert_ne!(*app.named_views.get("Work view").unwrap(), first);
    enter(&mut app, "NamedView Delete Front detail");
    enter(&mut app, "NamedView List");
    assert_eq!(app.command_log.back().unwrap(), "Named views: Work view");
}

#[test]
fn named_view_commands_leave_a_partial_modeling_prompt_and_redo_intact() {
    let mut app = test_app();
    for input in ["Point 1,2,3", "Undo", "Line", "0"] {
        enter(&mut app, input);
    }
    let pending = app.active_command;
    let drafting_plane = app.drafting_plane;
    enter(&mut app, "NamedView Save Line start");
    enter(&mut app, "NamedView Restore Line start");
    assert_eq!(app.active_command, pending);
    assert_eq!(app.drafting_plane, drafting_plane);
    assert!(app.document.can_redo());
    assert_eq!(app.document.objects().len(), 0);
    enter(&mut app, "1,2");
    assert!(app.active_command.is_none());
    assert_eq!(app.document.objects().len(), 1);
}

#[test]
fn named_views_round_trip_through_app_3dm_commands() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-named-views-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path_text = path.display().to_string();
    let mut source = test_app();
    enter(&mut source, "SetView World Perspective");
    enter(&mut source, "CPlane World Front");
    enter(&mut source, "NamedView Save Camera A");
    let saved = source.viewports[0].named_view_snapshot();
    enter(&mut source, &format!("Export3dm \"{path_text}\""));
    assert!(
        source
            .command_log
            .back()
            .unwrap()
            .contains("exported 1 named view(s)")
    );
    let model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(model.named_views.len(), 1);
    assert_eq!(model.named_views[0].name, "Camera A");
    assert_eq!(
        model.named_views[0].construction_plane,
        source.viewports[0].construction_plane()
    );

    let mut destination = test_app();
    enter(&mut destination, "NamedView Save Camera A");
    enter(&mut destination, &format!("Import3dm \"{path_text}\""));
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .contains("imported 1 named view(s)")
    );
    assert!(destination.named_views.get("Camera A (2)").is_ok());
    enter(&mut destination, "NamedView Restore Camera A (2)");
    let actual = Viewport::named_view_to_3dm(
        destination.viewports[0].named_view_snapshot(),
        "Camera A".to_owned(),
    )
    .unwrap();
    let expected = Viewport::named_view_to_3dm(saved, "Camera A".to_owned()).unwrap();
    assert_eq!(actual.construction_plane, expected.construction_plane);
    assert_eq!(actual.projection, expected.projection);
    for (actual, expected) in actual
        .camera_location
        .to_array()
        .into_iter()
        .zip(expected.camera_location.to_array())
    {
        assert!((actual - expected).abs() < 1.0e-8);
    }
    for (actual, expected) in actual.frustum.into_iter().zip(expected.frustum) {
        assert!((actual - expected).abs() < 1.0e-8);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn open_restores_current_viewports_without_named_views() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-current-views-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = test_app();
    enter(&mut source, "SetView World Perspective");
    source.viewports[0].display_mode = DisplayMode::Shaded;
    source.viewports[1].display_mode = DisplayMode::Ghosted;
    source.active_viewport = 1;
    source.viewports[1].set_grid_settings(GridSettings {
        snap_spacing: 0.25,
        minor_spacing: 2.5,
        major_interval: 8,
        line_count: 23,
        show_grid: false,
        show_axes: false,
        show_world_axes: true,
    });
    enter(&mut source, "MaxViewport");
    assert_eq!(source.maximized_viewport, Some(1));
    let expected = source.three_dm_viewports().unwrap();
    enter(&mut source, &format!("SaveAs \"{}\"", path.display()));
    assert!(
        source.command_log.back().unwrap().starts_with("Saved"),
        "{:?}",
        source.command_log.back()
    );
    let model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert!(model.named_views.is_empty());
    assert_eq!(model.viewports.len(), 4);
    assert_eq!(
        model.viewports[0].display_mode,
        viboceros_io::ThreeDmDisplayMode::Shaded
    );
    assert_eq!(
        model.viewports[1].display_mode,
        viboceros_io::ThreeDmDisplayMode::Ghosted
    );
    assert_eq!(model.viewports[1].grid.snap_spacing, 0.25);
    assert_eq!(model.viewports[1].grid.minor_spacing, 2.5);
    assert!(model.viewports[1].active);
    assert!(model.viewports[1].maximized);
    let mut reordered = model.clone();
    let saved_positions = [
        [0.0, 0.3, 0.0, 0.7],
        [0.3, 1.0, 0.0, 0.4],
        [0.0, 0.3, 0.7, 1.0],
        [0.3, 1.0, 0.4, 1.0],
    ];
    for (view, position) in reordered.viewports.iter_mut().zip(saved_positions) {
        view.position = position;
    }
    reordered.viewports.reverse();
    viboceros_io::write_3dm_file(&path, &reordered).unwrap();

    let mut destination = test_app();
    enter(&mut destination, &format!("Open \"{}\"", path.display()));
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .contains("opened 0 named view(s)")
    );
    assert_eq!(destination.viewports[0].kind(), ViewKind::Perspective);
    assert_eq!(destination.viewports[0].display_mode, DisplayMode::Shaded);
    assert_eq!(destination.viewports[1].display_mode, DisplayMode::Ghosted);
    assert_eq!(destination.active_viewport, 1);
    assert_eq!(destination.maximized_viewport, Some(1));
    assert_eq!(destination.viewport_positions, saved_positions);
    assert_eq!(
        destination.viewports[1].grid_settings(),
        source.viewports[1].grid_settings()
    );
    let actual = destination.three_dm_viewports().unwrap();
    for (actual, expected) in actual.iter().zip(expected.iter()) {
        assert_eq!(actual.camera.projection, expected.camera.projection);
        for (actual, expected) in actual
            .camera
            .camera_location
            .to_array()
            .into_iter()
            .zip(expected.camera.camera_location.to_array())
        {
            assert!((actual - expected).abs() < 1.0e-6);
        }
        for (actual, expected) in actual
            .camera
            .frustum
            .into_iter()
            .zip(expected.camera.frustum)
        {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }
    enter(&mut destination, "4View");
    assert_eq!(destination.maximized_viewport, None);
    assert_eq!(destination.active_viewport, 1);
    let export = path.with_file_name(format!(
        "viboceros-resaved-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    enter(
        &mut destination,
        &format!("Export3dm \"{}\"", export.display()),
    );
    let reexported = viboceros_io::read_3dm_file(&export, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        reexported
            .viewports
            .iter()
            .map(|view| view.position)
            .collect::<Vec<_>>(),
        saved_positions
    );
    std::fs::remove_file(export).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn read_viewports_from_file_preserves_document_and_converts_units() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-read-views-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = test_app();
    enter(&mut source, "Units Meters Scale=No");
    enter(&mut source, "SetView World Perspective");
    source.viewports[0].display_mode = DisplayMode::Ghosted;
    source.viewports[0].set_grid_settings(GridSettings {
        snap_spacing: 0.25,
        minor_spacing: 2.5,
        major_interval: 8,
        line_count: 23,
        show_grid: false,
        show_axes: false,
        show_world_axes: true,
    });
    source.active_viewport = 2;
    enter(&mut source, "MaxViewport");
    source.viewport_positions = vec![
        [0.0, 0.3, 0.0, 0.7],
        [0.3, 1.0, 0.0, 0.4],
        [0.0, 0.3, 0.7, 1.0],
        [0.3, 1.0, 0.4, 1.0],
    ];
    enter(&mut source, &format!("Export3dm \"{}\"", path.display()));
    assert!(source.command_log.back().unwrap().starts_with("Exported"));
    let expected = viboceros_io::read_3dm_viewports_file_in_units(
        &path,
        &viboceros_io::LengthUnitSystem::Millimeters,
    )
    .unwrap();
    assert_eq!(expected.len(), 4);
    assert_eq!(expected[0].grid.snap_spacing, 250.0);

    let mut destination = test_app();
    enter(&mut destination, "Point 9,8,7");
    enter(&mut destination, "NamedView Save Existing view");
    enter(&mut destination, "Line");
    enter(&mut destination, "0");
    let pending = destination.active_command;
    let path_before = destination.document_path.clone();
    enter(
        &mut destination,
        &format!("ReadViewportsFromFile \"{}\"", path.display()),
    );
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .starts_with("Read 4")
    );
    assert_eq!(destination.document_path, path_before);
    assert_eq!(destination.document.objects().len(), 1);
    assert!(destination.named_views.get("Existing view").is_ok());
    assert_eq!(destination.active_command, pending);
    assert!(destination.document.can_undo());
    assert_eq!(destination.viewports[0].kind(), ViewKind::Perspective);
    assert_eq!(destination.viewports[0].display_mode, DisplayMode::Ghosted);
    assert_eq!(destination.viewports[0].grid_settings().snap_spacing, 250.0);
    assert_eq!(destination.active_viewport, 2);
    assert_eq!(destination.maximized_viewport, Some(2));
    assert_eq!(destination.viewport_positions, source.viewport_positions);
    let actual = destination.three_dm_viewports().unwrap();
    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(actual.camera.projection, expected.camera.projection);
        for (actual, expected) in actual
            .camera
            .target
            .unwrap()
            .to_array()
            .into_iter()
            .zip(expected.camera.target.unwrap().to_array())
        {
            assert!((actual - expected).abs() < 1.0e-4, "viewport {index}");
        }
        if index == 0 {
            for (actual, expected) in actual
                .camera
                .camera_location
                .to_array()
                .into_iter()
                .zip(expected.camera.camera_location.to_array())
            {
                assert!((actual - expected).abs() < 1.0e-4);
            }
        }
    }

    let views_before = destination.three_dm_viewports().unwrap();
    enter(&mut destination, "ReadViewportsFromFile /missing/model.3dm");
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .starts_with("Error:")
    );
    assert_eq!(destination.three_dm_viewports().unwrap(), views_before);
    let empty_path = path.with_file_name(format!(
        "viboceros-no-views-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut empty_model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    empty_model.viewports.clear();
    viboceros_io::write_3dm_file(&empty_path, &empty_model).unwrap();
    enter(
        &mut destination,
        &format!("ReadViewportsFromFile \"{}\"", empty_path.display()),
    );
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .contains("No model viewports")
    );
    assert_eq!(destination.three_dm_viewports().unwrap(), views_before);
    std::fs::remove_file(empty_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn file_viewport_titles_survive_open_read_and_export() {
    let directory = std::env::temp_dir().join(format!(
        "viboceros-viewport-titles-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let input = directory.join("input.3dm");
    let output = directory.join("output.3dm");
    let mut source = test_app();
    enter(&mut source, &format!("Export3dm \"{}\"", input.display()));
    let mut model = viboceros_io::read_3dm_file(&input, Tolerance::DEFAULT).unwrap();
    let titles = [
        "Studio",
        "Oblique Review",
        "North Elevation",
        "South Detail",
    ];
    for (viewport, title) in model.viewports.iter_mut().zip(titles) {
        viewport.camera.name = title.to_owned();
    }
    viboceros_io::write_3dm_file(&input, &model).unwrap();

    let mut opened = test_app();
    enter(&mut opened, &format!("Open \"{}\"", input.display()));
    assert_eq!(
        opened
            .viewports
            .iter()
            .map(Viewport::view_label)
            .collect::<Vec<_>>(),
        titles.to_vec()
    );
    enter(&mut opened, "SetActiveViewport \"north elevation\"");
    assert_eq!(opened.active_viewport, 2);
    enter(&mut opened, &format!("Export3dm \"{}\"", output.display()));
    let exported = viboceros_io::read_3dm_file(&output, Tolerance::DEFAULT).unwrap();
    assert_eq!(
        exported
            .viewports
            .iter()
            .map(|viewport| viewport.camera.name.as_str())
            .collect::<Vec<_>>(),
        titles
    );

    let mut read = test_app();
    enter(
        &mut read,
        &format!("ReadViewportsFromFile \"{}\"", input.display()),
    );
    assert_eq!(
        read.viewports
            .iter()
            .map(Viewport::view_label)
            .collect::<Vec<_>>(),
        titles.to_vec()
    );
    enter(&mut read, "SetMaximizedViewport South Detail");
    assert_eq!(read.active_viewport, 3);
    assert_eq!(read.maximized_viewport, Some(3));
    assert!(!read.viewports[3].title_modified());
    enter(&mut read, "SetView World Top");
    assert_eq!(read.viewports[3].view_label(), "South Detail");
    assert!(read.viewports[3].title_modified());

    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn three_view_layout_round_trips_and_read_viewports_changes_count() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-three-view-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = test_app();
    enter(&mut source, "3View");
    source.viewports[2].display_mode = DisplayMode::Shaded;
    source.active_viewport = 2;
    enter(&mut source, &format!("SaveAs \"{}\"", path.display()));
    let model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(model.viewports.len(), 3);
    assert_eq!(
        model
            .viewports
            .iter()
            .map(|view| view.position)
            .collect::<Vec<_>>(),
        THREE_VIEWPORT_POSITIONS.to_vec()
    );

    let mut opened = test_app();
    enter(&mut opened, &format!("Open \"{}\"", path.display()));
    assert_eq!(opened.viewports.len(), 3);
    assert_eq!(opened.viewport_positions, THREE_VIEWPORT_POSITIONS.to_vec());
    assert_eq!(opened.active_viewport, 2);
    assert_eq!(opened.viewports[2].display_mode, DisplayMode::Shaded);

    let mut read = test_app();
    enter(
        &mut read,
        &format!("ReadViewportsFromFile \"{}\"", path.display()),
    );
    assert_eq!(read.viewports.len(), 3);
    assert_eq!(read.viewport_positions, THREE_VIEWPORT_POSITIONS.to_vec());
    assert!(read.document_path.is_none());
    enter(&mut read, "4View");
    assert_eq!(read.viewports.len(), 4);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn open_accepts_more_than_four_saved_model_viewports() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-five-view-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = test_app();
    enter(&mut source, &format!("Export3dm \"{}\"", path.display()));
    let mut model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    let mut extra = model.viewports[1].clone();
    extra.camera.name = "Detail Five".into();
    extra.active = false;
    extra.maximized = false;
    model.viewports.push(extra);
    let positions = [
        [0.0, 1.0 / 3.0, 0.0, 0.5],
        [1.0 / 3.0, 2.0 / 3.0, 0.0, 0.5],
        [2.0 / 3.0, 1.0, 0.0, 0.5],
        [0.0, 0.5, 0.5, 1.0],
        [0.5, 1.0, 0.5, 1.0],
    ];
    for (view, position) in model.viewports.iter_mut().zip(positions) {
        view.position = position;
    }
    viboceros_io::write_3dm_file(&path, &model).unwrap();

    let mut opened = test_app();
    enter(&mut opened, &format!("Open \"{}\"", path.display()));
    assert_eq!(opened.viewports.len(), 5);
    assert_eq!(opened.viewport_positions, positions.to_vec());
    assert_eq!(opened.viewports[4].view_label(), "Detail Five");
    enter(&mut opened, "SetMaximizedViewport Detail Five");
    assert_eq!(opened.active_viewport, 4);
    assert_eq!(opened.maximized_viewport, Some(4));
    assert_eq!(opened.three_dm_viewports().unwrap().len(), 5);
    let mut read = test_app();
    enter(
        &mut read,
        &format!("ReadViewportsFromFile \"{}\"", path.display()),
    );
    assert_eq!(read.viewports.len(), 5);
    assert_eq!(read.viewport_positions, positions.to_vec());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn open_3dm_replaces_session_document_and_named_views() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-open-view-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = test_app();
    enter(&mut source, "Point 1,2,3");
    enter(&mut source, "SetView World Perspective");
    enter(&mut source, "NamedView Save File camera");
    enter(&mut source, &format!("Export3dm \"{}\"", path.display()));

    let mut destination = test_app();
    enter(&mut destination, "Point 99,0,0");
    enter(&mut destination, "NamedView Save Old camera");
    enter(&mut destination, &format!("Open \"{}\"", path.display()));
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .contains("opened 1 named view(s)")
    );
    assert_eq!(destination.document.objects().len(), 1);
    assert_eq!(
        destination.named_views.names().collect::<Vec<_>>(),
        vec!["File camera"]
    );
    assert!(!destination.document.can_undo());
    assert!(destination.last_point.is_none());
    enter(&mut destination, "NamedView Restore File camera");
    assert_eq!(destination.viewports[0].kind(), ViewKind::Perspective);
    enter(&mut destination, "Open3dm /missing/model.3dm");
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .starts_with("Error:")
    );
    assert_eq!(destination.document.objects().len(), 1);
    assert!(destination.named_views.get("File camera").is_ok());
    assert_eq!(destination.document_path.as_deref(), Some(path.as_path()));
    enter(&mut destination, "Point 4,5,6");
    enter(&mut destination, "Save");
    let saved = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(saved.objects.len(), 2);
    assert_eq!(saved.named_views.len(), 1);
    assert!(path.with_extension("3dmbak").exists());
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_file(path.with_extension("3dmbak")).unwrap();
}

#[test]
fn save_and_save_as_track_the_active_3dm_without_rebinding_exports() {
    let directory = std::env::temp_dir().join(format!(
        "viboceros-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let first = directory.join("first file.3dm");
    let second = directory.join("second.3dm");
    let export = directory.join("export.3dm");
    let mut app = test_app();
    enter(&mut app, "Save");
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert!(app.document_path.is_none());

    enter(&mut app, "Point 1,2,3");
    enter(&mut app, "NamedView Save Work view");
    enter(
        &mut app,
        &format!("SaveAs \"{}\"", directory.join("first file").display()),
    );
    assert_eq!(app.document_path.as_deref(), Some(first.as_path()));
    let initial = viboceros_io::read_3dm_file(&first, Tolerance::DEFAULT).unwrap();
    assert_eq!(initial.objects.len(), 1);
    assert_eq!(initial.named_views.len(), 1);

    enter(&mut app, "Point 4,5,6");
    enter(&mut app, &format!("Export3dm \"{}\"", export.display()));
    assert_eq!(app.document_path.as_deref(), Some(first.as_path()));
    enter(&mut app, "Save");
    assert_eq!(
        viboceros_io::read_3dm_file(&first, Tolerance::DEFAULT)
            .unwrap()
            .objects
            .len(),
        2
    );
    assert_eq!(
        viboceros_io::read_3dm_file(first.with_extension("3dmbak"), Tolerance::DEFAULT)
            .unwrap()
            .objects
            .len(),
        1
    );
    assert_eq!(app.document.undo_label(), Some("Point"));

    enter(&mut app, &format!("SaveAs \"{}\"", second.display()));
    assert_eq!(app.document_path.as_deref(), Some(second.as_path()));
    enter(&mut app, "Point 7,8,9");
    enter(&mut app, "Save");
    assert_eq!(
        viboceros_io::read_3dm_file(&first, Tolerance::DEFAULT)
            .unwrap()
            .objects
            .len(),
        2
    );
    assert_eq!(
        viboceros_io::read_3dm_file(&second, Tolerance::DEFAULT)
            .unwrap()
            .objects
            .len(),
        3
    );

    enter(
        &mut app,
        &format!("SaveAs {}", directory.join("missing/fail.3dm").display()),
    );
    assert!(app.command_log.back().unwrap().starts_with("Error:"));
    assert_eq!(app.document_path.as_deref(), Some(second.as_path()));
    std::fs::remove_dir_all(directory).unwrap();
}
