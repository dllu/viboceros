use super::named_view_policy::{check_view, from_row, viewport_from_row};
use super::*;
use viboceros_io::ThreeDmProjection;

fn observations() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/view_camera_cplane_two_point.json"
    ))
    .unwrap()
}

fn input_row(row: &serde_json::Value) -> serde_json::Value {
    let mut before = row.clone();
    for field in [
        "camera_location",
        "camera_direction",
        "camera_up",
        "camera_target",
        "perspective",
        "two_point_perspective",
        "frustum",
        "cplane_origin",
        "cplane_x",
        "cplane_y",
    ] {
        before[field] = row[format!("{field}_before")].clone();
    }
    before
}

fn check_frustum(actual: [f64; 6], expected: [f64; 6], perspective: bool) {
    // Rhino recomputes clipping from the document and CPlane grid. Compare
    // optical projection independently of that remaining clipping work.
    let (a_near, b_near) = if perspective {
        (actual[4], expected[4])
    } else {
        (1., 1.)
    };
    for (a, b) in actual[..4].iter().zip(&expected[..4]) {
        assert!(
            (a / a_near - b / b_near).abs() < 2e-12,
            "frustum: {a} vs {b}"
        );
    }
}

#[test]
fn cplane_and_plan_transitions_match_64_rhino_camera_captures() {
    let capture = observations();
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/view_camera_cplane_two_point.json"
    ))
    .unwrap();
    assert_eq!(
        request["operations"].as_array().unwrap().len(),
        capture["results"].as_array().unwrap().len()
    );
    let mut count = 0;
    let mut off_axis = 0;
    for (operation, result) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(capture["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], result["id"]);
        assert_eq!(
            result["value"].as_array().unwrap().len(),
            operation["directions"].as_array().unwrap().len()
        );
        for row in result["value"].as_array().unwrap() {
            let before_row = input_row(row);
            let mut view = viewport_from_row(&before_row);
            let initial =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            check_view(&initial, &from_row(&before_row), true);
            for (a, b) in initial
                .frustum
                .into_iter()
                .zip(from_row(&before_row).frustum)
            {
                assert!((a - b).abs() < 2e-12);
            }
            off_axis += usize::from(view.camera_target_offset.norm() > 1e-8);
            let before = view.camera_snapshot();
            let plane = view.construction_plane();
            let view_history = view.view_undo.len();
            let plane_history = view.grid_undo.len();
            let is_cplane_view = row["direction"] == "CPlaneView";
            match row["direction"].as_str().unwrap() {
                "CPlaneView" => {
                    view.set_construction_plane(view.construction_plane_aligned_to_view().unwrap());
                }
                "Plan" => view.set_plan_view().unwrap(),
                direction => view.set_cplane_view(
                    WorldPlane::ALL
                        .into_iter()
                        .find(|kind| kind.label() == direction)
                        .unwrap(),
                ),
            }
            let actual =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            let expected = from_row(row);
            check_view(
                &actual,
                &expected,
                expected.projection != ThreeDmProjection::Parallel,
            );
            check_frustum(
                actual.frustum,
                expected.frustum,
                row["perspective"].as_bool().unwrap(),
            );
            let rect = view.last_rect.unwrap();
            // Side CPlane views look along the current drafting plane, where
            // a parallel ray has no unique plane intersection. Check each
            // camera's drafting ray against a plane normal to the view.
            let mut drafting = view.duplicate_for_layout("Drafting ray");
            drafting.plane =
                ConstructionPlaneState::new(view.construction_plane_aligned_to_view().unwrap());
            let mut checked = 0;
            for query in row["projected_points"].as_array().unwrap() {
                let model = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(query["point"].clone()).unwrap(),
                )
                .unwrap();
                let Some(screen) = view.project_precise(model, rect) else {
                    continue;
                };
                let expected: [f64; 2] = serde_json::from_value(query["screen"].clone()).unwrap();
                for (a, b) in screen.into_iter().zip(expected) {
                    assert!(
                        (a - b).abs() < 1e-8,
                        "{} {} screen: {a} vs {b}",
                        result["id"],
                        row["direction"]
                    );
                }
                let pixel = view.project(model, rect).unwrap();
                let depth = view.view_depth(model);
                let (gpu, _) = gpu_project(&view, rect, model, (depth - 1., depth + 1.));
                let rounding = 4. * f32::EPSILON * pixel.x.abs().max(pixel.y.abs()).max(1.);
                assert!(gpu.distance(pixel) <= rounding.max(1e-3));
                let picked = drafting
                    .unproject_drafting_plane(pixel, rect, Some(model))
                    .unwrap();
                assert!(picked.distance_to(model).unwrap() < 1e-4);
                checked += 1;
            }
            assert!(checked >= 2);
            if is_cplane_view {
                assert_eq!(view.camera_snapshot(), before);
                assert_eq!(view.view_undo.len(), view_history);
                assert!(view.undo_construction_plane());
                assert_eq!(view.construction_plane(), plane);
                assert_eq!(view.camera_snapshot(), before);
                assert!(view.redo_construction_plane());
            } else {
                assert_eq!(view.construction_plane(), plane);
                assert_eq!(view.grid_undo.len(), plane_history);
                let after = view.camera_snapshot();
                assert!(view.undo_view());
                assert_eq!(view.camera_snapshot(), before);
                assert!(view.redo_view());
                assert_eq!(view.camera_snapshot(), after);
            }
            count += 1;
        }
    }
    assert_eq!(count, 64);
    assert!(off_axis >= 6);
}

#[test]
fn off_axis_camera_targets_and_shifted_frusta_survive_3dm_files() {
    let capture = observations();
    let mut model = viboceros_io::ThreeDmModel::new(Vec::new(), Vec::new(), Vec::new());
    for result in capture["results"].as_array().unwrap() {
        for row in result["value"].as_array().unwrap() {
            for input in [input_row(row), row.clone()] {
                let mut view = from_row(&input);
                view.name = format!("Camera {}", model.named_views.len());
                model.named_views.push(view);
            }
        }
    }
    let path = std::env::temp_dir().join(format!(
        "viboceros-cplane-cameras-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    viboceros_io::write_3dm_file(&path, &model).unwrap();
    let loaded = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(loaded.named_views.len(), 128);
    for (actual, expected) in loaded.named_views.iter().zip(&model.named_views) {
        let encoded = Viewport::named_view_to_3dm(
            Viewport::named_view_from_3dm(actual).unwrap(),
            String::new(),
        )
        .unwrap();
        check_view(&encoded, expected, true);
        for (a, b) in encoded.frustum.into_iter().zip(expected.frustum) {
            assert!((a - b).abs() < 2e-12);
        }
    }
}

#[test]
fn plan_conversion_uses_the_target_depth_beyond_near_without_clamping_to_far() {
    for (near, far, distance) in [
        (100., 200., 50.),
        (50., 200., 50.),
        (25., 200., 50.),
        (25., 40., 50.),
    ] {
        let mut view = Viewport::new(ViewKind::Perspective);
        view.frustum_near = near;
        view.frustum_far = far;
        view.perspective_camera_distance = distance;
        view.perspective_lens_shift = [0.2, -0.4];
        view.camera_target_offset = NaVector3::new(2., 3., 0.);
        let before = view.camera_snapshot();
        let factor = distance.max(near) / near;
        let old = Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
        view.set_plan_view().unwrap();
        let actual =
            Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
        assert_eq!(actual.projection, ThreeDmProjection::Parallel);
        assert!(
            ((actual.frustum[3] - actual.frustum[2]) / (old.frustum[3] - old.frustum[2]) - factor)
                .abs()
                < 1e-14
        );
        assert_eq!(view.camera_target_offset, NaVector3::zeros());
        assert_eq!(actual.target, Some(view.construction_plane().origin()));
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), before);
    }
}

#[test]
fn plan_rejects_unrepresentable_scale_without_changing_camera_or_histories() {
    for (near, fov) in [(1e100, 0.5), (1., 1e-100)] {
        let mut view = Viewport::new(ViewKind::Perspective);
        view.frustum_near = near;
        view.frustum_far = near * 100.;
        view.perspective_fov_radians = fov;
        let before = view.named_view_snapshot();
        assert!(view.set_plan_view().is_err());
        assert_eq!(view.named_view_snapshot(), before);
        assert!(view.view_undo.is_empty());
        assert!(view.grid_undo.is_empty());
    }
}

#[test]
fn explicit_target_and_extents_commands_replace_imported_target_offsets() {
    let capture = observations();
    let mut checked = [false; 2];
    for result in capture["results"].as_array().unwrap() {
        for row in result["value"].as_array().unwrap() {
            let mut view = viewport_from_row(&input_row(row));
            let slot = usize::from(view.kind == ViewKind::Perspective);
            if checked[slot] || view.camera_target_offset.norm() < 1e-8 {
                continue;
            }
            let before = view.camera_snapshot();
            let rect = view.last_rect.unwrap();
            let target = point(25., -10., 19.);
            let corner = view.project(target, rect).unwrap() + Vec2::new(30., 20.);
            assert!(view.zoom_target(target, corner, rect).unwrap());
            let encoded =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            assert_eq!(encoded.target, Some(target));
            assert_eq!(
                view.construction_plane_aligned_to_view().unwrap().origin(),
                target
            );
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            let mut model = Document::default();
            model.add_geometry(Geometry::Point(target)).unwrap();
            assert!(
                view.zoom_extents(&model, ZoomExtentsBorders::default())
                    .unwrap()
            );
            let encoded =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            assert_eq!(encoded.target, Some(target));
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            checked[slot] = true;
        }
    }
    assert_eq!(checked, [true; 2]);
}
