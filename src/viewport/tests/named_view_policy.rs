use super::*;
use viboceros_command::named_view::NamedViewPolicy;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn position(value: &serde_json::Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

fn vector(value: &serde_json::Value) -> Vector3 {
    Vector3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}

fn projection(row: &serde_json::Value) -> ThreeDmProjection {
    if row["two_point_perspective"].as_bool().unwrap() {
        ThreeDmProjection::TwoPointPerspective
    } else if row["perspective"].as_bool().unwrap() {
        ThreeDmProjection::Perspective
    } else {
        ThreeDmProjection::Parallel
    }
}

fn policy(value: &serde_json::Value) -> NamedViewPolicy {
    NamedViewPolicy {
        set_cplane: value["set_cplane"].as_bool().unwrap(),
        set_projection: value["set_projection"].as_bool().unwrap(),
    }
}

pub(super) fn from_row(row: &serde_json::Value) -> ThreeDmNamedView {
    let [width, height]: [i32; 2] = serde_json::from_value(row["viewport_size"].clone()).unwrap();
    ThreeDmNamedView {
        name: "Rhino captured view".into(),
        projection: projection(row),
        camera_location: position(&row["camera_location"]),
        camera_direction: vector(&row["camera_direction"]),
        camera_up: vector(&row["camera_up"]),
        target: Some(position(&row["camera_target"])),
        construction_plane: Frame3::try_from_directions(
            position(&row["cplane_origin"]),
            vector(&row["cplane_x"]),
            vector(&row["cplane_y"]),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        frustum: serde_json::from_value(row["frustum"].clone()).unwrap(),
        screen_port: [0, width, height, 0],
    }
}

pub(super) fn viewport_from_row(row: &serde_json::Value) -> Viewport {
    let source = from_row(row);
    let mut view = Viewport::new(ViewKind::Top);
    view.restore_named_view(Viewport::named_view_from_3dm(&source).unwrap());
    view.last_rect = Some(Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(source.screen_port[1] as f32, source.screen_port[2] as f32),
    ));
    view
}

fn check_vector(actual: [f64; 3], expected: [f64; 3], context: &str) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!((a - b).abs() < 2e-12, "{context}: {a} vs {b}");
    }
}

pub(super) fn check_view(actual: &ThreeDmNamedView, expected: &ThreeDmNamedView, location: bool) {
    assert_eq!(actual.projection, expected.projection);
    if location {
        check_vector(
            actual.camera_location.to_array(),
            expected.camera_location.to_array(),
            "location",
        );
    }
    check_vector(
        actual.camera_direction.to_array(),
        expected.camera_direction.to_array(),
        "direction",
    );
    check_vector(
        actual.camera_up.to_array(),
        expected.camera_up.to_array(),
        "up",
    );
    check_vector(
        actual.target.unwrap().to_array(),
        expected.target.unwrap().to_array(),
        "target",
    );
    check_vector(
        actual.construction_plane.origin().to_array(),
        expected.construction_plane.origin().to_array(),
        "CPlane origin",
    );
    check_vector(
        actual.construction_plane.x_axis().as_vector().to_array(),
        expected.construction_plane.x_axis().as_vector().to_array(),
        "CPlane x",
    );
    check_vector(
        actual.construction_plane.y_axis().as_vector().to_array(),
        expected.construction_plane.y_axis().as_vector().to_array(),
        "CPlane y",
    );
}

fn check_points(view: &Viewport, row: &serde_json::Value) {
    let rect = view.last_rect.unwrap();
    let mut checked = 0;
    for query in row["projected_points"].as_array().unwrap() {
        // Rendering discards points behind perspective cameras. Rhino's
        // WorldToClient also returns coordinates for those points.
        let Some(actual) = view.project_precise(position(&query["point"]), rect) else {
            assert_eq!(view.kind, ViewKind::Perspective);
            continue;
        };
        let expected: [f64; 2] = serde_json::from_value(query["screen"].clone()).unwrap();
        for (a, b) in actual.into_iter().zip(expected) {
            assert!((a - b).abs() < 1e-8, "screen: {a} vs {b}");
        }
        checked += 1;
    }
    assert!(checked >= 2);
}

#[test]
fn named_view_policy_matches_72_saved_rhino_restores() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/named_view_policy.json"
    ))
    .unwrap();
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/named_view_policy.json"
    ))
    .unwrap();
    let mut count = 0;
    for (operation, result) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(capture["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], result["id"]);
        let rows = result["value"].as_array().unwrap();
        assert_eq!(rows.len(), 9);
        for row in rows {
            let saved = Viewport::named_view_from_3dm(&from_row(&row["saved"])).unwrap();
            let mut view = viewport_from_row(&row["before"]);
            let before = view.camera_snapshot();
            let plane = view.construction_plane();
            let plane_history = view.grid_undo.len();
            view.restore_named_view_with_policy(saved, policy(&row["view_policy"]))
                .unwrap();
            let actual =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            let expected = from_row(&row["after"]);
            check_view(&actual, &expected, true);
            for (a, b) in actual.frustum.into_iter().zip(expected.frustum) {
                assert!((a - b).abs() < 2e-12, "frustum: {a} vs {b}");
            }
            check_points(&view, &row["after"]);
            let after = view.camera_snapshot();
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            assert!(view.redo_view());
            assert_eq!(view.camera_snapshot(), after);
            if !policy(&row["view_policy"]).set_cplane {
                assert_eq!(view.construction_plane(), plane);
                assert_eq!(view.grid_undo.len(), plane_history);
            } else {
                assert!(view.undo_construction_plane());
                assert_eq!(view.construction_plane(), plane);
                assert_eq!(view.camera_snapshot(), after);
                assert!(view.redo_construction_plane());
            }
            let round_trip = Viewport::named_view_to_3dm(
                Viewport::named_view_from_3dm(&actual).unwrap(),
                String::new(),
            )
            .unwrap();
            check_view(&round_trip, &actual, true);
            for (a, b) in round_trip.frustum.into_iter().zip(actual.frustum) {
                assert!((a - b).abs() < 2e-12);
            }
            count += 1;
        }
    }
    assert_eq!(count, 72);
}

#[test]
fn world_view_policy_matches_96_saved_rhino_presets() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/view_camera_world_policy.json"
    ))
    .unwrap();
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/view_camera_world_policy.json"
    ))
    .unwrap();
    let mut count = 0;
    for (operation, result) in request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(capture["results"].as_array().unwrap())
    {
        assert_eq!(operation["id"], result["id"]);
        for row in result["value"].as_array().unwrap() {
            let mut source = row.clone();
            for field in [
                "camera_location",
                "camera_direction",
                "camera_up",
                "camera_target",
                "frustum",
                "perspective",
                "two_point_perspective",
            ] {
                source[field] = row[format!("{field}_before")].clone();
            }
            source["cplane_origin"] = operation["origin"].clone();
            source["cplane_x"] = operation["x_axis"].clone();
            source["cplane_y"] = operation["y_axis"].clone();
            assert_eq!(
                source["perspective"].as_bool().unwrap(),
                row["projection"] != "Top"
            );
            assert_eq!(
                source["two_point_perspective"].as_bool().unwrap(),
                row["projection"] == "TwoPointPerspective"
            );
            let mut view = viewport_from_row(&source);
            let before = view.camera_snapshot();
            let plane_history = view.grid_undo.len();
            let policy = policy(&row["view_policy"]);
            match row["direction"].as_str().unwrap() {
                "WorldPerspective" => view.set_world_perspective_view_with_policy(false, policy),
                "WorldTwoPointPerspective" => {
                    view.set_world_perspective_view_with_policy(true, policy)
                }
                other => {
                    let kind = match other {
                        "WorldTop" => ViewKind::Top,
                        "WorldBottom" => ViewKind::Bottom,
                        "WorldFront" => ViewKind::Front,
                        "WorldBack" => ViewKind::Back,
                        "WorldRight" => ViewKind::Right,
                        "WorldLeft" => ViewKind::Left,
                        _ => panic!("unexpected world preset {other}"),
                    };
                    view.set_world_view_with_policy(kind, policy)
                }
            }
            .unwrap();
            let actual =
                Viewport::named_view_to_3dm(view.named_view_snapshot(), String::new()).unwrap();
            let expected = from_row(row);
            // Parallel clipping can move the camera along its depth axis.
            check_view(
                &actual,
                &expected,
                expected.projection != ThreeDmProjection::Parallel,
            );
            for (a, b) in actual.frustum[..4].iter().zip(&expected.frustum[..4]) {
                let (a, b) = if expected.projection == ThreeDmProjection::Parallel {
                    (*a, *b)
                } else {
                    (a / actual.frustum[4], b / expected.frustum[4])
                };
                assert!((a - b).abs() < 2e-12, "frustum: {a} vs {b}");
            }
            check_points(&view, row);
            if !policy.set_cplane {
                assert_eq!(view.grid_undo.len(), plane_history);
            }
            let after = view.camera_snapshot();
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            assert!(view.redo_view());
            assert_eq!(view.camera_snapshot(), after);
            count += 1;
        }
    }
    assert_eq!(count, 96);
}

#[test]
fn unrepresentable_named_projection_conversion_leaves_camera_and_histories_intact() {
    let mut saved = Viewport::new(ViewKind::Perspective);
    saved.frustum_near = 1e-308;
    saved.plane.set(WorldPlane::Left.frame());
    let mut destination = Viewport::new(ViewKind::Top);
    destination.set_view_title("Current");
    let before = destination.named_view_snapshot();
    let policy = NamedViewPolicy {
        set_cplane: true,
        set_projection: false,
    };
    assert!(
        destination
            .restore_named_view_with_policy(saved.named_view_snapshot(), policy)
            .is_err()
    );
    assert_eq!(destination.named_view_snapshot(), before);
    assert!(destination.view_undo.is_empty());
    assert!(destination.grid_undo.is_empty());
    assert!(!destination.title_modified());
}

#[test]
fn restored_shifted_frusta_survive_actual_3dm_files() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/observations/named_view_policy.json"
    ))
    .unwrap();
    let mut model = viboceros_io::ThreeDmModel::new(Vec::new(), Vec::new(), Vec::new());
    for result in capture["results"].as_array().unwrap() {
        for row in result["value"].as_array().unwrap() {
            let view = viewport_from_row(&row["after"]);
            model.named_views.push(
                Viewport::named_view_to_3dm(
                    view.named_view_snapshot(),
                    format!("Policy view {}", model.named_views.len()),
                )
                .unwrap(),
            );
        }
    }
    let path = std::env::temp_dir().join(format!(
        "viboceros-policy-frusta-{}-{}.3dm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    viboceros_io::write_3dm_file(&path, &model).unwrap();
    let decoded = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(decoded.named_views.len(), model.named_views.len());
    for (actual, expected) in decoded.named_views.iter().zip(&model.named_views) {
        check_view(actual, expected, true);
        for (a, b) in actual.frustum.into_iter().zip(expected.frustum) {
            assert!((a - b).abs() < 2e-12);
        }
        let again = Viewport::named_view_to_3dm(
            Viewport::named_view_from_3dm(actual).unwrap(),
            String::new(),
        )
        .unwrap();
        check_view(&again, expected, true);
    }
}
