use super::*;
use viboceros_geometry::Vector3;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.to_owned();
    app.run_command();
}

#[test]
fn named_cplane_restores_plane_and_grid_without_moving_camera_or_model() {
    let mut app = test_app();
    let frame = Frame3::try_from_directions(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(1.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(-1.0, 1.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    app.viewports[0].plane.set(frame);
    let grid = GridSettings {
        snap_spacing: 0.25,
        minor_spacing: 2.5,
        major_interval: 0,
        line_count: 23,
        show_grid: false,
        show_axes: false,
        show_world_axes: true,
    };
    app.viewports[0].set_grid_settings(grid);
    enter(&mut app, "NamedCPlane Save Detail plane");
    enter(&mut app, "NamedCPlane Duplicate Detail plane | Copy");
    enter(&mut app, "NamedCPlane MoveUp Copy");
    assert_eq!(
        app.named_cplanes.names().collect::<Vec<_>>(),
        ["Copy", "Detail plane"]
    );

    app.active_viewport = 1;
    let camera = app.viewports[1].camera_snapshot();
    let original_plane = app.viewports[1].construction_plane();
    let original_grid = app.viewports[1].grid_settings();
    enter(&mut app, "NamedCPlane Restore detail PLANE");
    assert_eq!(app.viewports[1].construction_plane(), frame);
    assert_eq!(app.viewports[1].camera_snapshot(), camera);
    let restored_grid = app.viewports[1].grid_settings();
    assert_eq!(restored_grid.snap_spacing, grid.snap_spacing);
    assert_eq!(restored_grid.minor_spacing, grid.minor_spacing);
    assert_eq!(restored_grid.major_interval, grid.major_interval);
    assert_eq!(restored_grid.line_count, grid.line_count);
    assert_eq!(restored_grid.show_grid, original_grid.show_grid);
    enter(&mut app, "CPlane Undo");
    assert_eq!(app.viewports[1].construction_plane(), original_plane);
    enter(&mut app, "CPlane Redo");
    assert_eq!(app.viewports[1].construction_plane(), frame);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "NamedCPlane Rename Copy | Archive");
    enter(&mut app, "NamedCPlane Delete Archive");
    enter(&mut app, "NamedCPlane List");
    assert_eq!(
        app.command_log.back().unwrap(),
        "Named CPlanes: Detail plane"
    );
}

#[test]
fn named_cplanes_persist_and_import_without_model_content() {
    let path = std::env::temp_dir().join(format!(
        "viboceros-named-cplane-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = test_app();
    enter(&mut source, "Units Meters Scale=No");
    enter(&mut source, "Point 4,5,6");
    let frame = Frame3::try_from_directions(
        Point3::try_new(1.0, 2.0, 3.0).unwrap(),
        Vector3::try_new(1.0, 1.0, 0.0).unwrap(),
        Vector3::try_new(-1.0, 1.0, 1.0).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    source.viewports[0].plane.set(frame);
    source.viewports[0].set_grid_settings(GridSettings {
        snap_spacing: 0.25,
        minor_spacing: 2.5,
        major_interval: 8,
        line_count: 23,
        ..GridSettings::default()
    });
    enter(&mut source, "NamedCPlane Save Detail");
    enter(&mut source, &format!("Export3dm \"{}\"", path.display()));
    let file = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    assert_eq!(file.named_cplanes.len(), 1);
    assert_eq!(file.named_cplanes[0].name, "Detail");
    assert_eq!(file.named_cplanes[0].plane, frame);

    let mut destination = test_app();
    enter(&mut destination, "Point 9,8,7");
    enter(&mut destination, "Undo");
    enter(&mut destination, "NamedCPlane Save Detail");
    let viewport_before = destination.viewports[0].camera_snapshot();
    let plane_before = destination.viewports[0].construction_plane();
    enter(
        &mut destination,
        &format!("NamedCPlane Import \"{}\"", path.display()),
    );
    assert!(
        destination
            .command_log
            .back()
            .unwrap()
            .contains("Imported 1")
    );
    assert_eq!(
        destination.named_cplanes.names().collect::<Vec<_>>(),
        ["Detail", "Detail (2)"]
    );
    assert_eq!(destination.document.objects().count(), 0);
    assert!(destination.document.can_redo());
    assert_eq!(destination.viewports[0].camera_snapshot(), viewport_before);
    assert_eq!(destination.viewports[0].construction_plane(), plane_before);
    let imported = destination.named_cplanes.get("Detail (2)").unwrap().clone();
    assert_eq!(imported.plane.origin().to_array(), [1000.0, 2000.0, 3000.0]);
    assert_eq!(imported.grid_spacing, 2500.0);
    assert_eq!(imported.snap_spacing, 250.0);
    enter(&mut destination, "NamedCPlane Restore Detail (2)");
    assert_eq!(
        destination.viewports[0].construction_plane().origin(),
        imported.plane.origin()
    );
    enter(
        &mut destination,
        &format!("Import3dm \"{}\"", path.display()),
    );
    assert!(destination.named_cplanes.get("Detail (3)").is_ok());
    assert_eq!(destination.document.objects().count(), 1);

    let mut opened = test_app();
    enter(&mut opened, &format!("Open3dm \"{}\"", path.display()));
    assert!(opened.named_cplanes.get("Detail").is_ok());
    assert_eq!(opened.document.objects().count(), 1);
    std::fs::remove_file(path).unwrap();
}
