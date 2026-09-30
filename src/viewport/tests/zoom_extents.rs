use super::named_view_policy::{check_view, from_row, viewport_from_row};
use super::*;

#[test]
fn zoom_extents_framing_and_perspective_poses_match_saved_rhino_captures() {
    check_capture(
        include_str!("../../../tools/rhino_oracle/fixtures/zoom_extents_camera.json"),
        include_str!("../../../tools/rhino_oracle/observations/zoom_extents_camera.json"),
        85,
        2,
    );
}

#[test]
fn zoom_extents_border_settings_below_one_match_rhino_api_and_command_captures() {
    check_capture(
        include_str!("../../../tools/rhino_oracle/fixtures/zoom_extents_borders.json"),
        include_str!("../../../tools/rhino_oracle/observations/zoom_extents_borders.json"),
        20,
        0,
    );
}

fn check_capture(request: &str, capture: &str, expected_count: usize, expected_small: usize) {
    let capture: serde_json::Value = serde_json::from_str(capture).unwrap();
    let request: serde_json::Value = serde_json::from_str(request).unwrap();
    let rows = capture["results"][0]["value"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        request["operations"][0]["cases"].as_array().unwrap().len()
    );
    let mut count = 0;
    let mut small_camera = 0;
    for (row, case) in rows
        .iter()
        .zip(request["operations"][0]["cases"].as_array().unwrap())
    {
        assert_eq!(row["case"], *case);
        let mut view = viewport_from_row(&row["before"]);
        let before = view.camera_snapshot();
        let plane = view.construction_plane();
        let plane_undo = view.grid_undo.len();
        let view_undo = view.view_undo.len();
        let minimum: [f64; 3] = serde_json::from_value(case["min"].clone()).unwrap();
        let maximum: [f64; 3] = serde_json::from_value(case["max"].clone()).unwrap();
        let mut document = Document::default();
        let mut ids = Vec::new();
        for index in 0..8 {
            let corner = Point3::try_from(std::array::from_fn(|axis| {
                if index & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }
            }))
            .unwrap();
            ids.push(document.add_geometry(Geometry::Point(corner)).unwrap());
        }
        document
            .select_objects(ids, SelectionMode::Replace)
            .unwrap();
        let objects = document.objects().cloned().collect::<Vec<_>>();
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let document_undo = document.undo_label().map(str::to_owned);
        let border = case["border"].as_f64().unwrap_or(1.);
        let borders = ZoomExtentsBorders {
            parallel: border,
            perspective: border,
        };
        if case["method"] == "Selected" {
            assert_eq!(view.zoom_selected(&document, borders), Ok(true));
        } else {
            assert_eq!(view.zoom_extents(&document, borders), Ok(true));
        }
        let actual =
            Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
        let expected = from_row(&row["after"]);
        // Automatic document clipping may additionally dolly parallel views.
        // Explicit bounding-box fits match the public camera and clip distances.
        check_view(
            &actual,
            &expected,
            view.kind == ViewKind::Perspective || case["method"] == "BoundingBox",
        );
        for index in 0..4 {
            let (a, b) = if view.kind == ViewKind::Perspective {
                (
                    actual.frustum[index] / actual.frustum[4],
                    expected.frustum[index] / expected.frustum[4],
                )
            } else {
                (actual.frustum[index], expected.frustum[index])
            };
            assert!(
                (a - b).abs() <= 2e-12 * b.abs().max(1.),
                "{} frustum {index}: {a} vs {b}",
                case["id"]
            );
        }
        if case["method"] == "BoundingBox" {
            for (a, b) in actual.frustum.into_iter().zip(expected.frustum) {
                assert!((a - b).abs() < 2e-12, "{} clip: {a} vs {b}", case["id"]);
            }
        }
        for query in row["after"]["projected_points"].as_array().unwrap() {
            let model = Point3::try_from(
                serde_json::from_value::<[f64; 3]>(query["point"].clone()).unwrap(),
            )
            .unwrap();
            let rect = view.last_rect.unwrap();
            let pixel = view.project_precise(model, rect).unwrap();
            let expected: [f64; 2] = serde_json::from_value(query["screen"].clone()).unwrap();
            for (a, b) in pixel.into_iter().zip(expected) {
                assert!((a - b).abs() < 1e-8, "{} screen: {a} vs {b}", case["id"]);
            }
            let screen = view.project(model, rect).unwrap();
            let depth = view.view_depth(model);
            let (gpu, _) = gpu_project(&view, rect, model, (depth - 1., depth + 1.));
            let rounding = 8. * f32::EPSILON * screen.x.abs().max(screen.y.abs()).max(1.);
            assert!(
                gpu.distance(screen) < rounding.max(1e-3),
                "{} GPU",
                case["id"]
            );
        }
        assert_eq!(view.perspective_fov_radians, before.perspective_fov_radians);
        assert_eq!(view.perspective_lens_shift, [0.; 2]);
        assert_eq!(view.parallel_frustum_shift, [0.; 2]);
        assert_eq!(view.camera_target_offset, NaVector3::zeros());
        assert_eq!(view.construction_plane(), plane);
        assert_eq!(view.grid_undo.len(), plane_undo);
        assert_eq!(view.view_undo.len(), view_undo + 1);
        small_camera += usize::from(
            view.kind == ViewKind::Perspective
                && view.perspective_camera_distance < MIN_PERSPECTIVE_CAMERA_DISTANCE,
        );
        let after = view.camera_snapshot();
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), before);
        assert!(view.redo_view());
        assert_eq!(view.camera_snapshot(), after);
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(document.undo_label(), document_undo.as_deref());
        count += 1;
    }
    assert_eq!(count, expected_count);
    assert_eq!(small_camera, expected_small);
}

#[test]
fn zoom_all_rejects_an_invalid_lens_without_mutating_cameras_or_history() {
    let mut document = Document::default();
    for p in [point(-1., -2., -3.), point(4., 5., 6.)] {
        document.add_geometry(Geometry::Point(p)).unwrap();
    }
    for invalid in [Real::NAN, Real::INFINITY, 0., -1., std::f64::consts::PI] {
        let mut views = [
            Viewport::new(ViewKind::Top),
            Viewport::new(ViewKind::Perspective),
        ];
        for view in &mut views {
            view.last_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.)));
            view.perspective_lens_shift = [0.4, -0.7];
            view.camera_target_offset = NaVector3::new(2., 3., 4.);
        }
        views[1].perspective_fov_radians = invalid;
        let previous = views.each_ref().map(|view| view.camera_snapshot());
        let histories = views
            .each_ref()
            .map(|view| (view.view_undo.clone(), view.view_redo.clone()));
        assert_eq!(
            Viewport::zoom_all(&mut views, &document, false, ZoomExtentsBorders::default()),
            Err("invalid perspective field of view")
        );
        for ((view, before), history) in views.iter().zip(previous).zip(histories) {
            // NaN in the deliberately invalid lens needs a bitwise comparison.
            assert_eq!(
                view.perspective_fov_radians.to_bits(),
                before.perspective_fov_radians.to_bits()
            );
            let mut actual = view.camera_snapshot();
            let mut before = before;
            actual.perspective_fov_radians = 1.;
            before.perspective_fov_radians = 1.;
            assert_eq!(actual, before);
            assert_eq!(view.view_undo, history.0);
            assert_eq!(view.view_redo, history.1);
        }
    }
}
