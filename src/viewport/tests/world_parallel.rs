use super::*;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn position(value: &serde_json::Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

fn vector(value: &serde_json::Value) -> Vector3 {
    Vector3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

#[test]
fn world_parallel_presets_match_saved_rhino_framing() {
    let observation: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/view_camera_world_parallel.json"
    ))
    .unwrap();
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/view_camera_world_parallel.json"
    ))
    .unwrap();
    let operations = request["operations"].as_array().unwrap();
    let results = observation["results"].as_array().unwrap();
    assert_eq!(operations.len(), results.len());
    for (operation, result) in operations.iter().zip(results) {
        assert_eq!(operation["id"], result["id"]);
        let rows = result["value"].as_array().unwrap();
        assert_eq!(
            rows.len(),
            operation["projections"].as_array().unwrap().len()
                * operation["directions"].as_array().unwrap().len()
        );
        for row in rows {
            let [width, height]: [i32; 2] =
                serde_json::from_value(row["viewport_size"].clone()).unwrap();
            let source = ThreeDmNamedView {
                name: "Rhino input".into(),
                projection: match row["projection"].as_str().unwrap() {
                    "Top" => ThreeDmProjection::Parallel,
                    "Perspective" => ThreeDmProjection::Perspective,
                    "TwoPointPerspective" => ThreeDmProjection::TwoPointPerspective,
                    other => panic!("unexpected projection {other}"),
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
            };
            let mut view = Viewport::new(ViewKind::Top);
            view.restore_named_view(Viewport::named_view_from_3dm(&source).unwrap());
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32));
            view.last_rect = Some(rect);
            let before = view.camera_snapshot();
            let kind = match row["direction"].as_str().unwrap() {
                "WorldTop" => ViewKind::Top,
                "WorldBottom" => ViewKind::Bottom,
                "WorldFront" => ViewKind::Front,
                "WorldBack" => ViewKind::Back,
                "WorldRight" => ViewKind::Right,
                "WorldLeft" => ViewKind::Left,
                other => panic!("unexpected world preset {other}"),
            };
            view.set_world_view(kind).unwrap();
            let actual =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), "Actual".into()).unwrap();
            assert_eq!(actual.projection, ThreeDmProjection::Parallel);
            for (value, field) in [
                (actual.camera_direction.to_array(), "camera_direction"),
                (actual.camera_up.to_array(), "camera_up"),
                (actual.target.unwrap().to_array(), "camera_target"),
                (
                    actual.construction_plane.origin().to_array(),
                    "cplane_origin",
                ),
                (
                    actual.construction_plane.x_axis().as_vector().to_array(),
                    "cplane_x",
                ),
                (
                    actual.construction_plane.y_axis().as_vector().to_array(),
                    "cplane_y",
                ),
            ] {
                let expected: [f64; 3] = serde_json::from_value(row[field].clone()).unwrap();
                for (a, b) in value.into_iter().zip(expected) {
                    assert!(
                        (a - b).abs() < 2e-12,
                        "{} {} {field}: {a} vs {b}",
                        operation["id"],
                        row["direction"]
                    );
                }
            }
            let frustum: [f64; 6] = serde_json::from_value(row["frustum"].clone()).unwrap();
            // Document clipping can relocate the parallel camera along its
            // depth axis. Test the framing independently of that pending work.
            for (a, b) in actual.frustum[..4].iter().zip(&frustum[..4]) {
                assert!((a - b).abs() < 2e-12, "frustum: {a} vs {b}");
            }
            for query in row["projected_points"].as_array().unwrap() {
                let model = position(&query["point"]);
                let precise = view.project_precise(model, rect).unwrap();
                let expected: [f64; 2] = serde_json::from_value(query["screen"].clone()).unwrap();
                for (a, b) in precise.into_iter().zip(expected) {
                    assert!((a - b).abs() < 1e-8, "screen: {a} vs {b}");
                }
                let pixel = view.project(model, rect).unwrap();
                let depth = view.view_depth(model);
                let (gpu, _) = gpu_project(&view, rect, model, (depth - 1., depth + 1.));
                let rounding = 4. * f32::EPSILON * pixel.x.abs().max(pixel.y.abs()).max(1.);
                assert!(gpu.distance(pixel) <= rounding.max(1e-3));
                let picked = view
                    .unproject_drafting_plane(pixel, rect, Some(model))
                    .unwrap();
                assert!(picked.distance_to(model).unwrap() < 1e-4);
            }
            let after = view.camera_snapshot();
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            assert!(view.redo_view());
            assert_eq!(view.camera_snapshot(), after);
            let imported = Viewport::named_view_from_3dm(&actual).unwrap();
            let again = Viewport::named_view_to_3dm(imported, "Round trip".into()).unwrap();
            assert_eq!(again.projection, actual.projection);
            assert_eq!(again.target, actual.target);
            assert_eq!(again.construction_plane, actual.construction_plane);
            for (a, b) in again.frustum.into_iter().zip(actual.frustum) {
                assert!((a - b).abs() < 2e-12);
            }
        }
    }
}

#[test]
fn world_parallel_presets_preserve_pan_center_and_clear_two_point_locks() {
    let mut view = Viewport::new(ViewKind::Front);
    view.target = NaVector3::new(10., 20., 30.);
    view.pan = Vec2::new(40., -80.);
    view.pixels_per_unit = 17.345_678_901_234;
    view.plane
        .set(WorldPlane::Left.frame().with_origin(point(7., 8., 9.)));
    let center = view.construction_plane_aligned_to_view().unwrap().origin();
    let scale = view.pixels_per_unit;
    view.set_world_view(ViewKind::Right).unwrap();
    assert_eq!(view.target, NaVector3::from(center.to_array()));
    assert_eq!(view.pan, Vec2::ZERO);
    assert_eq!(view.pixels_per_unit, scale);
    assert_eq!(
        view.construction_plane(),
        WorldPlane::Right.frame().with_origin(point(7., 8., 9.))
    );
    view.set_world_perspective_view(true).unwrap();
    assert!(view.two_point_perspective);
    view.set_world_view(ViewKind::Bottom).unwrap();
    assert!(!view.two_point_perspective);
    assert_eq!(view.kind, ViewKind::Bottom);
    assert!(view.undo_view());
    assert!(view.two_point_perspective);
}
