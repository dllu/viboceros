use super::*;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn vector(value: &serde_json::Value) -> Vector3 {
    Vector3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

fn position(value: &serde_json::Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

fn before_camera(row: &serde_json::Value, operation: &serde_json::Value) -> ThreeDmNamedView {
    let [width, height]: [i32; 2] = serde_json::from_value(row["viewport_size"].clone()).unwrap();
    ThreeDmNamedView {
        name: "Oracle input".into(),
        projection: if row["projection"] == "Top" {
            ThreeDmProjection::Parallel
        } else {
            ThreeDmProjection::Perspective
        },
        camera_location: position(&row["camera_location_before"]),
        camera_direction: vector(&row["camera_direction_before"]),
        camera_up: vector(&row["camera_up_before"]),
        target: Some(position(&row["camera_target_before"])),
        construction_plane: Frame3::try_from_directions(
            position(&operation["origin"]),
            vector(&operation["x_axis"]),
            vector(&operation["y_axis"]),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        frustum: serde_json::from_value(row["frustum_before"].clone()).unwrap(),
        screen_port: [0, width, height, 0],
    }
}

fn compare_camera(view: &Viewport, row: &serde_json::Value, rect: Rect) {
    let camera = Viewport::named_view_to_3dm(view.named_view_snapshot(), "Actual".into()).unwrap();
    for (actual, field) in [
        (camera.camera_location.to_array(), "camera_location"),
        (camera.camera_direction.to_array(), "camera_direction"),
        (camera.camera_up.to_array(), "camera_up"),
        (camera.target.unwrap().to_array(), "camera_target"),
        (
            camera.construction_plane.origin().to_array(),
            "cplane_origin",
        ),
        (
            camera.construction_plane.x_axis().as_vector().to_array(),
            "cplane_x",
        ),
        (
            camera.construction_plane.y_axis().as_vector().to_array(),
            "cplane_y",
        ),
    ] {
        let expected: [f64; 3] = serde_json::from_value(row[field].clone()).unwrap();
        for (a, b) in actual.into_iter().zip(expected) {
            assert!((a - b).abs() < 2e-12, "{field}: {a} != {b}");
        }
    }
    assert_eq!(
        camera.projection == ThreeDmProjection::TwoPointPerspective,
        row["two_point_perspective"].as_bool().unwrap()
    );
    let expected: [f64; 6] = serde_json::from_value(row["frustum"].clone()).unwrap();
    // Rhino recomputes document clipping distances after navigation. Compare
    // the optical projection independently of those near/far choices.
    for index in 0..4 {
        assert!(
            (camera.frustum[index] / camera.frustum[4] - expected[index] / expected[4]).abs()
                < 2e-12,
            "frustum component {index}"
        );
    }
    for query in row["projected_points"].as_array().unwrap() {
        let model = position(&query["point"]);
        let actual = view.project(model, rect).unwrap();
        let expected: [f64; 2] = serde_json::from_value(query["screen"].clone()).unwrap();
        assert!(
            (f64::from(actual.x) - expected[0]).abs() < 1e-4,
            "screen x: {} != {}",
            actual.x,
            expected[0]
        );
        assert!(
            (f64::from(actual.y) - expected[1]).abs() < 1e-4,
            "screen y: {} != {}",
            actual.y,
            expected[1]
        );
        let depth = view.view_depth(model);
        let (gpu, _) = gpu_project(view, rect, model, (depth - 1., depth + 1.));
        assert!(
            gpu.distance(actual) < 1e-3,
            "CPU/GPU screen difference: {actual:?} vs {gpu:?}"
        );
        if camera.projection == ThreeDmProjection::TwoPointPerspective && model.z() == 0. {
            let picked = view
                .unproject_drafting_plane(actual, rect, Some(model))
                .unwrap();
            assert!(
                picked.distance_to(model).unwrap() < 1e-4,
                "drafting pick {picked:?} vs {model:?}"
            );
        }
    }
}

#[test]
fn world_perspective_presets_and_two_point_drags_match_saved_rhino_cameras() {
    let observation: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/view_camera_two_point.json"
    ))
    .unwrap();
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/view_camera_two_point.json"
    ))
    .unwrap();
    for (operation, result) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(observation["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], result["id"]);
        for row in result["value"].as_array().unwrap() {
            let source = before_camera(row, operation);
            let mut view = Viewport::new(ViewKind::Top);
            view.restore_named_view(Viewport::named_view_from_3dm(&source).unwrap());
            let rect = Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(source.screen_port[1] as f32, source.screen_port[2] as f32),
            );
            view.last_rect = Some(rect);
            let before = view.camera_snapshot();
            view.set_world_perspective_view(row["direction"] == "WorldTwoPointPerspective")
                .unwrap();
            compare_camera(&view, row, rect);
            let after = view.camera_snapshot();
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            assert!(view.redo_view());
            assert_eq!(view.camera_snapshot(), after);
            if let Some(drag) = operation.get("mouse_drag") {
                let delta: [f32; 2] = serde_json::from_value(drag.clone()).unwrap();
                view.apply_navigation_drag(
                    PointerButton::Secondary,
                    egui::Modifiers::default(),
                    Vec2::new(delta[0], delta[1]),
                );
                compare_camera(&view, &row["navigation_after"], rect);
                let saved = Viewport::named_view_to_3dm(
                    view.named_view_snapshot(),
                    "Two point after drag".into(),
                )
                .unwrap();
                let mut imported = Viewport::new(ViewKind::Top);
                imported.restore_named_view(Viewport::named_view_from_3dm(&saved).unwrap());
                imported.last_rect = Some(rect);
                compare_camera(&imported, &row["navigation_after"], rect);
            }
        }
    }
}

#[test]
fn two_point_world_preset_retains_translated_parallel_pan() {
    let mut view = Viewport::new(ViewKind::Front);
    view.target = NaVector3::new(10., 20., 30.);
    view.pan = Vec2::new(40., -80.);
    let center = view.construction_plane_aligned_to_view().unwrap().origin();
    let plane = view.construction_plane();
    view.set_world_perspective_view(true).unwrap();
    assert_eq!(view.target, NaVector3::from(center.to_array()));
    assert_eq!(view.pan, Vec2::ZERO);
    assert_eq!(
        view.construction_plane(),
        WorldPlane::Top.frame().with_origin(plane.origin())
    );
    view.set_plan_view().unwrap();
    assert!(!view.two_point_perspective);
    assert!(view.undo_view());
    assert!(view.two_point_perspective);
}
