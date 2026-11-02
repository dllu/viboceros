//! Cached native frames survive actual named-view and working-view file commands.
use super::*;
use serde_json::Value;

fn enter(app: &mut VibocerosApp, text: &str) {
    app.command_input = text.into();
    app.run_command();
}
fn close(a: [f64; 3], b: [f64; 3]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 2e-12, "{a} vs {b}");
    }
}
fn check(app: &VibocerosApp, native: &Value) -> usize {
    let view = &app.viewports[0];
    let camera = Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
    for (axis, expected) in camera
        .camera_frame()
        .unwrap()
        .axes()
        .into_iter()
        .zip(native["frame"].as_array().unwrap())
    {
        close(
            axis.as_vector().to_array(),
            serde_json::from_value(expected.clone()).unwrap(),
        );
    }
    close(
        camera.camera_location.to_array(),
        serde_json::from_value(native["location"].clone()).unwrap(),
    );
    assert_eq!(
        camera.camera_direction.to_array(),
        serde_json::from_value::<[f64; 3]>(native["direction"].clone()).unwrap()
    );
    assert_eq!(
        camera.camera_up.to_array(),
        serde_json::from_value::<[f64; 3]>(native["up"].clone()).unwrap()
    );
    let rect = egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::Vec2::new(camera.screen_port[1] as f32, camera.screen_port[2] as f32),
    );
    let projection = view.pick_projection().unwrap();
    let mut visible = 0;
    for row in native["points"].as_array().unwrap() {
        let point =
            Point3::try_from(serde_json::from_value::<[f64; 3]>(row["world"].clone()).unwrap())
                .unwrap();
        let Some(screen) = view.project(point, rect) else {
            continue;
        };
        let expected: [f64; 2] = serde_json::from_value(row["screen"].clone()).unwrap();
        for (actual, expected) in [screen.x, screen.y].into_iter().zip(expected) {
            let tolerance = 2. * f64::from(f32::EPSILON) * expected.abs().max(1.);
            assert!(
                (f64::from(actual) - expected).abs() <= tolerance,
                "screen={screen:?}, expected={expected}, camera={camera:?}"
            );
        }
        let pick = projection.continuous_point(point).unwrap();
        assert!((pick.x() - expected[0]).abs() < 1e-8);
        assert!((f64::from(rect.height()) - pick.y() - expected[1]).abs() < 1e-8);
        visible += 1;
    }
    visible
}

#[test]
fn camera_archive_named_and_current_views_match_native_frames_pixels_and_picking() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let mut rows = Vec::new();
    for family in ["camera_archive", "camera_archive_projection"] {
        let n: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("observations/{family}.json"))).unwrap(),
        )
        .unwrap();
        rows.extend(n["results"].as_array().unwrap().iter().cloned());
    }
    let path = std::env::temp_dir().join(format!(
        "viboceros-cached-camera-{}.3dm",
        std::process::id()
    ));
    let mut count = 0;
    let mut skew = 0;
    for row in &rows {
        let value = &row["value"];
        let input = root.join(value["file"].as_str().unwrap());
        let model = viboceros_io::read_3dm_file(&input, Tolerance::DEFAULT).unwrap();
        let mut app = test_app();
        enter(&mut app, "Point 1,2,3");
        let id = app.document.objects().next().unwrap().id();
        let geometry = app.document.object(id).unwrap().geometry_snapshot().clone();
        // Supply the native viewport dimensions as the offscreen layout context.
        app.viewports[0]
            .restore_file_camera(&model.named_views[0])
            .unwrap();
        enter(&mut app, &format!("Import3dm \"{}\"", input.display()));
        assert!(
            app.command_log
                .back()
                .unwrap()
                .contains("imported 1 named view(s)"),
            "{:?}",
            app.command_log.back()
        );
        enter(&mut app, "SetView World Top");
        enter(&mut app, "NamedView Restore Owned archive view");
        let visible = check(&app, &value["named_restore"]);
        // The first capture has one view with every fixed sample behind it.
        // Its supplemental archive supplies native camera-relative samples.
        if visible == 0 {
            assert_eq!(row["id"], "TwoPointPerspective-tilted-1");
        }
        assert_eq!(
            app.document.object(id).unwrap().geometry_snapshot(),
            &geometry
        );
        // Cached locked frames must stay distinct from a direction-priority
        // projection derived by discarding the native archive's frame.
        let camera = &model.named_views[0];
        let raw = camera
            .camera_direction
            .normalized_nonzero()
            .unwrap()
            .as_vector();
        let optical = camera.camera_frame().unwrap().z_axis().as_vector();
        if raw.dot(optical).unwrap().abs() < 1. - 1e-10 {
            skew += 1;
            assert_eq!(
                camera.projection,
                viboceros_io::ThreeDmProjection::TwoPointPerspective
            );
        }
        enter(&mut app, &format!("SaveAs \"{}\"", path.display()));
        assert!(
            app.command_log.back().unwrap().starts_with("Saved"),
            "{:?}",
            app.command_log.back()
        );
        let mut opened = test_app();
        enter(&mut opened, &format!("Open \"{}\"", path.display()));
        assert_eq!(check(&opened, &value["source"]), visible);
        enter(&mut opened, "SetView World Top");
        enter(&mut opened, "NamedView Restore Owned archive view");
        assert_eq!(check(&opened, &value["named_restore"]), visible);
        count += 1;
    }
    std::fs::remove_file(path).unwrap();
    assert_eq!(count, 20);
    assert_eq!(skew, 4);
}
