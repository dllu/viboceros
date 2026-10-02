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

#[test]
fn document_clipping_and_public_constraints_match_rhino_captures() {
    check_capture(
        include_str!("../../../tools/rhino_oracle/fixtures/viewport_clipping.json"),
        include_str!("../../../tools/rhino_oracle/observations/viewport_clipping.json"),
        81,
        2,
    );
}

#[test]
fn selected_fits_include_visible_document_context_in_all_display_modes() {
    check_capture(
        include_str!("../../../tools/rhino_oracle/fixtures/viewport_clipping_context.json"),
        include_str!("../../../tools/rhino_oracle/observations/viewport_clipping_context.json"),
        72,
        0,
    );
}

#[test]
fn box_depths_match_48_rhino_queries_including_clipped_and_shifted_frusta() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/viewport_box_depth.json"
    ))
    .unwrap();
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/viewport_box_depth.json"
    ))
    .unwrap();
    let rows = capture["results"][0]["value"].as_array().unwrap();
    assert_eq!(rows.len(), 48);
    let mut misses = 0;
    for (row, case) in rows
        .iter()
        .zip(request["operations"][0]["cases"].as_array().unwrap())
    {
        assert_eq!(row["case"], *case);
        let view = viewport_from_row(&row["before"]);
        let bounds = viboceros_geometry::BoundingBox3::from_points([
            Point3::try_from(
                serde_json::from_value::<[f64; 3]>(case["depth_min"].clone()).unwrap(),
            )
            .unwrap(),
            Point3::try_from(
                serde_json::from_value::<[f64; 3]>(case["depth_max"].clone()).unwrap(),
            )
            .unwrap(),
        ])
        .unwrap();
        let actual = view.bounding_box_depth(bounds, view.last_rect.unwrap());
        let expected = &row["clipping"]["depth_query"];
        assert_eq!(
            actual.is_some(),
            expected["intersects"].as_bool().unwrap(),
            "{}",
            case["id"]
        );
        if let Some((near, far)) = actual {
            for (a, b) in [
                (near, expected["near"].as_f64().unwrap()),
                (far, expected["far"].as_f64().unwrap()),
            ] {
                assert!(
                    (a - b).abs() <= 2e-12 * b.abs().max(1.),
                    "{} box depth: {a} vs {b}",
                    case["id"]
                );
            }
        } else {
            misses += 1;
        }
    }
    assert!(misses >= 6);
}

#[test]
fn perspective_box_depth_rejects_boxes_on_or_behind_the_camera_plane() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
    let mut view = Viewport::new(ViewKind::Perspective);
    view.perspective_frame = Some(WorldPlane::Top.frame());
    view.perspective_camera_distance = 10.;
    for (low, high) in [(10., 10.), (10., 11.), (11., 12.)] {
        let bounds = viboceros_geometry::BoundingBox3::from_points([
            point(-1., -1., low),
            point(1., 1., high),
        ])
        .unwrap();
        assert_eq!(view.bounding_box_depth(bounds, rect), None);
    }
    let bounds =
        viboceros_geometry::BoundingBox3::from_points([point(-1., -1., 9.), point(1., 1., 10.)])
            .unwrap();
    let (near, far) = view.bounding_box_depth(bounds, rect).unwrap();
    assert_eq!(near, 0.);
    // Corner-ray intersection conservatively expands the far endpoint using
    // the public camera-coordinate tolerance.
    let tolerance = Real::EPSILON.sqrt() * 11.;
    assert!((far - (1. + tolerance)).abs() < 1e-12);
}

#[test]
fn redraw_clipping_matches_navigation_visibility_and_empty_scene_captures() {
    check_redraw_capture(
        include_str!("../../../tools/rhino_oracle/observations/viewport_clipping_redraw.json"),
        189,
    );
    check_redraw_capture(
        include_str!("../../../tools/rhino_oracle/observations/viewport_clipping_fallback.json"),
        24,
    );
}

fn check_redraw_capture(capture: &str, expected_count: usize) {
    let capture: serde_json::Value = serde_json::from_str(capture).unwrap();
    let mut checked = 0;
    for row in capture["results"][0]["value"].as_array().unwrap() {
        let mut document = Document::default();
        let minimum: [f64; 3] = serde_json::from_value(row["case"]["min"].clone()).unwrap();
        let maximum: [f64; 3] = serde_json::from_value(row["case"]["max"].clone()).unwrap();
        for index in 0..8 {
            document
                .add_geometry(Geometry::Point(
                    Point3::try_from(std::array::from_fn(|axis| {
                        if index & (1 << axis) == 0 {
                            minimum[axis]
                        } else {
                            maximum[axis]
                        }
                    }))
                    .unwrap(),
                ))
                .unwrap();
        }
        for step in row["clipping"]["redraw_steps"].as_array().unwrap() {
            if let Some(visible) = step["action"].get("visible") {
                let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
                document
                    .set_objects_visibility(ids, visible.as_bool().unwrap())
                    .unwrap();
            } else if step["action"].get("delete_geometry").is_some() {
                let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
                for id in ids {
                    document.delete_object(id).unwrap();
                }
            }
            let mut view = viewport_from_row(&step["before"]);
            let before = view.camera_snapshot();
            view.view_undo.push(before);
            view.view_redo.push(before);
            let histories = (view.view_undo.clone(), view.view_redo.clone());
            assert!(
                view.refresh_clipping(&document, view.last_rect.unwrap())
                    .unwrap()
            );
            let actual =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            let expected = from_row(&step["after"]);
            check_view(&actual, &expected, true);
            for (a, b) in actual.frustum.into_iter().zip(expected.frustum) {
                assert!(
                    (a - b).abs() < 2e-12 * b.abs().max(1.),
                    "{} {:?}: {a} vs {b}",
                    row["case"]["id"],
                    step["action"]
                );
            }
            assert_eq!(view.view_undo, histories.0);
            assert_eq!(view.view_redo, histories.1);
            for query in step["after"]["projected_points"].as_array().unwrap() {
                let point = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(query["point"].clone()).unwrap(),
                )
                .unwrap();
                let Some(actual) = view.project_precise(point, view.last_rect.unwrap()) else {
                    // WorldToClient returns coordinates behind the camera;
                    // display projection intentionally culls those points.
                    assert_eq!(view.kind, ViewKind::Perspective);
                    assert!(view.view_depth(point) <= 0.);
                    continue;
                };
                let expected: [f64; 2] = serde_json::from_value(query["screen"].clone()).unwrap();
                for (a, b) in actual.into_iter().zip(expected) {
                    assert!(
                        (a - b).abs() < 1e-8,
                        "{} screen: {a} vs {b}",
                        row["case"]["id"]
                    );
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, expected_count);
}

#[test]
fn drawing_refreshes_clipping_and_keeps_camera_model_and_plane_history_separate() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/viewport_clipping_redraw.json"
    ))
    .unwrap();
    let row = &capture["results"][0]["value"][0];
    let mut view = viewport_from_row(&row["clipping"]["redraw_steps"][0]["before"]);
    let mut document = Document::default();
    for p in [point(-10., -5., -3.), point(10., 5., 3.)] {
        document.add_geometry(Geometry::Point(p)).unwrap();
    }
    let objects = document.objects().cloned().collect::<Vec<_>>();
    let history = document.undo_label().map(str::to_owned);
    let before = view.camera_snapshot();
    view.view_undo.push(before);
    view.view_redo.push(before);
    let histories = (view.view_undo.clone(), view.view_redo.clone());
    let context = egui::Context::default();
    let size = view.last_rect.unwrap().size();
    context
        .run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| {
                view.show(ui, &document, ViewportInput::default(), &[], 0, false);
            },
        )
        .drop_without_applying_deltas();
    assert_eq!(
        view.frustum_near,
        row["clipping"]["redraw_steps"][0]["after"]["frustum"][4]
            .as_f64()
            .unwrap()
    );
    assert_eq!(
        view.frustum_far,
        row["clipping"]["redraw_steps"][0]["after"]["frustum"][5]
            .as_f64()
            .unwrap()
    );
    assert_eq!(view.view_undo, histories.0);
    assert_eq!(view.view_redo, histories.1);
    assert!(view.grid_undo.is_empty() && view.grid_redo.is_empty());
    assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
    assert_eq!(document.undo_label(), history.as_deref());
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
        if let Some(minimum) = case.get("context_min") {
            let minimum: [f64; 3] = serde_json::from_value(minimum.clone()).unwrap();
            let maximum: [f64; 3] = serde_json::from_value(case["context_max"].clone()).unwrap();
            for index in 0..8 {
                let corner = Point3::try_from(std::array::from_fn(|axis| {
                    if index & (1 << axis) == 0 {
                        minimum[axis]
                    } else {
                        maximum[axis]
                    }
                }))
                .unwrap();
                document
                    .add_geometry_with_attributes(
                        Geometry::Point(corner),
                        ObjectAttributes::on_layer(document.current_layer_id())
                            .with_visibility(!case["context_hidden"].as_bool().unwrap()),
                    )
                    .unwrap();
            }
            view.display_mode = match case["display_mode"].as_str().unwrap() {
                "Wireframe" => DisplayMode::Wireframe,
                "Shaded" => DisplayMode::Shaded,
                "Ghosted" => DisplayMode::Ghosted,
                _ => unreachable!(),
            };
        }
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
        } else if case["method"] == "BoundingBox" {
            // Exercise the explicit fit independently of document clipping,
            // just as RhinoViewport.ZoomBoundingBox does.
            let bounds = viboceros_geometry::BoundingBox3::from_points([
                Point3::try_from(minimum).unwrap(),
                Point3::try_from(maximum).unwrap(),
            ])
            .unwrap();
            view.zoom_bounding_box(bounds).unwrap();
        } else {
            assert_eq!(view.zoom_extents(&document, borders), Ok(true));
        }
        let actual =
            Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
        let expected = from_row(&row["after"]);
        check_view(&actual, &expected, true);
        if let Some(redraw) = row["clipping"].get("after_redraw") {
            for (a, b) in actual.frustum.into_iter().zip(expected.frustum) {
                assert!(
                    (a - b).abs() <= 2e-12 * b.abs().max(1.),
                    "{} initial document clip: {a} vs {b}",
                    case["id"]
                );
            }
            // Redraw recalculates the box/frustum tolerance using the relocated
            // parallel camera. Compare this second stage separately from the
            // command's initial camera fit, without adding a history entry.
            let mut refreshed = view.duplicate_for_layout("clip refresh");
            refreshed.last_rect = view.last_rect;
            let histories = (refreshed.view_undo.clone(), refreshed.view_redo.clone());
            refreshed
                .update_clipping_from_document(&document, view.last_rect.unwrap())
                .unwrap();
            let info = Viewport::named_view_to_3dm(refreshed.named_view_snapshot(), String::new())
                .unwrap();
            let expected = from_row(redraw);
            check_view(&info, &expected, true);
            for (a, b) in info.frustum.into_iter().zip(expected.frustum) {
                assert!(
                    (a - b).abs() <= 2e-12 * b.abs().max(1.),
                    "{} redraw document clip: {a} vs {b}",
                    case["id"]
                );
            }
            assert_eq!(refreshed.view_undo, histories.0);
            assert_eq!(refreshed.view_redo, histories.1);
        }
        if let Some(values) = case.get("clip_constraints") {
            use crate::viewport::clipping::{ClipAdjustment, ClipRequest};
            let [near, far, min_near, min_ratio, target_distance]: [f64; 5] =
                serde_json::from_value(values.clone()).unwrap();
            let previous = view.camera_snapshot();
            let half_width = (actual.frustum[1] - actual.frustum[0]) * 0.5;
            let half_height = (actual.frustum[3] - actual.frustum[2]) * 0.5;
            let update = ClipAdjustment::constrained(
                ClipRequest {
                    near,
                    far,
                    min_near,
                    min_ratio,
                    target_distance,
                },
                view.kind == ViewKind::Perspective,
                half_width,
                half_height,
            );
            assert_eq!(
                update.is_ok(),
                row["clipping"]["succeeded"].as_bool().unwrap(),
                "{}",
                case["id"]
            );
            if let Ok(update) = update {
                view.apply_clip_adjustment(update).unwrap();
                let info =
                    Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
                let expected = &row["clipping"]["after"];
                for (a, b) in info
                    .frustum
                    .into_iter()
                    .zip(serde_json::from_value::<[f64; 6]>(expected["frustum"].clone()).unwrap())
                {
                    assert!(
                        (a - b).abs() <= 2e-12 * b.abs().max(1.),
                        "{} constraint clip: {a} vs {b}",
                        case["id"]
                    );
                }
                for (a, b) in info.camera_location.to_array().into_iter().zip(
                    serde_json::from_value::<[f64; 3]>(expected["camera_location"].clone())
                        .unwrap(),
                ) {
                    assert!((a - b).abs() < 2e-12, "{} dolly: {a} vs {b}", case["id"]);
                }
                assert_eq!(info.target.unwrap(), actual.target.unwrap());
            }
            view.restore_camera(previous);
        }
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
