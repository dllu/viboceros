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
    enter(&mut app, "NamedView Restore Upper left");
    assert_eq!(app.viewports[1].named_view_snapshot(), saved);
    assert_eq!(app.viewports[1].display_mode, DisplayMode::Ghosted);
    assert_eq!(app.viewports[0].named_view_snapshot(), saved);
    assert_eq!(app.document.undo_label(), None);
    assert!(app.viewports[1].undo_view());
    assert_eq!(app.viewports[1].camera_snapshot(), other_before);
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
