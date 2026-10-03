use super::*;
use viboceros_geometry::LineSegment;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

pub(super) fn captures() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../tools/rhino_oracle/observations/viewport_clip_transform.json"
    ))
    .unwrap()
}

pub(crate) fn captured_view(camera: &serde_json::Value) -> Viewport {
    let array = |key: &str| serde_json::from_value::<[f64; 3]>(camera[key].clone()).unwrap();
    let [width, height]: [i32; 2] =
        serde_json::from_value(camera["viewport_size"].clone()).unwrap();
    let source = ThreeDmNamedView {
        name: "Captured clip camera".into(),
        projection: if camera["two_point_perspective"].as_bool().unwrap() {
            ThreeDmProjection::TwoPointPerspective
        } else if camera["perspective"].as_bool().unwrap() {
            ThreeDmProjection::Perspective
        } else {
            ThreeDmProjection::Parallel
        },
        camera_location: Point3::try_from(array("camera_location")).unwrap(),
        camera_direction: Vector3::try_from(array("camera_direction")).unwrap(),
        camera_up: Vector3::try_from(array("camera_up")).unwrap(),
        target: Some(Point3::try_from(array("camera_target")).unwrap()),
        construction_plane: Frame3::try_from_directions(
            Point3::try_from(array("cplane_origin")).unwrap(),
            Vector3::try_from(array("cplane_x")).unwrap(),
            Vector3::try_from(array("cplane_y")).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        frustum: serde_json::from_value(camera["frustum"].clone()).unwrap(),
        screen_port: [0, width, height, 0],
    };
    let mut view = Viewport::new(ViewKind::Top);
    view.restore_named_view(Viewport::named_view_from_3dm(&source).unwrap());
    view.last_rect = Some(Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(width as f32, height as f32),
    ));
    view
}

#[test]
fn gpu_projection_and_clip_intervals_match_public_rhino_queries() {
    let capture = captures();
    let rows = capture["results"][0]["value"].as_array().unwrap();
    assert_eq!(rows.len(), 48);
    let mut checked = 0;
    let mut visibility_checked = 0;
    for row in rows {
        let view = captured_view(&row["projection"]["camera"]);
        let rect = view.last_rect.unwrap();
        let queries = row["projection"]["queries"].as_array().unwrap();
        let points: Vec<Point3> = queries
            .iter()
            .map(|query| {
                Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(query["point"].clone()).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let depths: Vec<_> = points.iter().map(|p| view.view_depth(*p)).collect();
        let range = Some((
            depths.iter().copied().fold(Real::INFINITY, Real::min),
            depths.iter().copied().fold(Real::NEG_INFINITY, Real::max),
        ));
        let uniform = view.gpu_view_uniform(rect, range);
        for ((query, point), depth) in queries.iter().zip(points).zip(depths) {
            let expected: [f64; 3] = serde_json::from_value(query["clip"].clone()).unwrap();
            let mut position = view.gpu_position(point).unwrap();
            view.encode_gpu_depth(&mut position, depth, range);
            let homogeneous: [f64; 4] = std::array::from_fn(|axis| {
                f64::from(uniform.view_projection[3][axis])
                    + (0..3)
                        .map(|i| {
                            f64::from(uniform.view_projection[i][axis]) * f64::from(position[i])
                        })
                        .sum::<f64>()
            });
            let actual = homogeneous.map(|v| v / homogeneous[3]);
            for axis in 0..2 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 2e-5,
                    "{} {} axis {axis}: {actual:?} vs {expected:?}",
                    row["case"]["id"],
                    query["id"]
                );
            }
            if view.kind == ViewKind::Perspective {
                assert!((actual[2] - (1. - expected[2]) * 0.5).abs() < 2e-5);
            }
            // Exact plane contacts may round to either side in either API;
            // compare classification only outside a narrow numeric edge band.
            if expected.iter().any(|v| (v.abs() - 1.).abs() < 2e-7) {
                continue;
            }
            let visible = actual[0].abs() <= 1.
                && actual[1].abs() <= 1.
                && actual[2] >= f64::from(uniform.clip_depth[0])
                && actual[2] <= f64::from(uniform.clip_depth[1]);
            assert_eq!(
                visible,
                query["visible"].as_bool().unwrap(),
                "{} {}",
                row["case"]["id"],
                query["id"]
            );
            visibility_checked += 1;
        }
        checked += queries.len();
    }
    assert_eq!(checked, 1200);
    assert!(
        visibility_checked >= 800,
        "checked {visibility_checked} visibility queries"
    );
}

#[test]
fn click_window_and_crossing_line_picks_match_public_rhino_contexts() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tools/rhino_oracle/observations/viewport_clipping_picks.json"
    ))
    .unwrap();
    let rows = capture["results"][0]["value"].as_array().unwrap();
    assert_eq!(rows.len(), 48);
    let mut checked = 0;
    for row in rows {
        let view = captured_view(&row["picking"]["camera"]);
        let viewport_rect = view.last_rect.unwrap();
        for pick in row["picking"]["picks"].as_array().unwrap() {
            let [x, y, width, height]: [f32; 4] =
                serde_json::from_value(pick["rect"].clone()).unwrap();
            let selection = Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height));
            for (index, line) in row["picking"]["lines"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let start = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(line["start"].clone()).unwrap(),
                )
                .unwrap();
                let end = Point3::try_from(
                    serde_json::from_value::<[f64; 3]>(line["end"].clone()).unwrap(),
                )
                .unwrap();
                let mut document = Document::default();
                let id = document
                    .add_geometry(Geometry::Line(
                        LineSegment::try_new(start, end, Tolerance::DEFAULT).unwrap(),
                    ))
                    .unwrap();
                let selected = match pick["style"].as_str().unwrap() {
                    "PointPick" => {
                        view.pick_object(selection.center(), viewport_rect, &document) == Some(id)
                    }
                    style => view
                        .objects_in_selection(
                            viewport_rect,
                            selection,
                            style == "CrossingPick",
                            &document,
                        )
                        .contains(&id),
                };
                assert_eq!(
                    selected,
                    pick["objects"][index].as_bool().unwrap(),
                    "{} {} {}",
                    row["case"]["id"],
                    pick["style"],
                    line["id"]
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 1296);
}

#[test]
fn point_cloud_clipped_nearest_members_do_not_suppress_visible_hits() {
    for kind in [
        ViewKind::Top,
        ViewKind::Bottom,
        ViewKind::Front,
        ViewKind::Back,
        ViewKind::Right,
        ViewKind::Left,
        ViewKind::Plan,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        view.frustum_near = 4.;
        view.frustum_far = 12.;
        view.pixels_per_unit = 40.;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.));
        let (right, up, forward) = if kind == ViewKind::Perspective {
            view.perspective_basis()
        } else {
            let frame = if kind == ViewKind::Plan {
                Frame3::try_from_directions(
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Vector3::try_new(1., 1., 0.).unwrap(),
                    Vector3::try_new(-1., 1., 1.).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap()
            } else {
                Viewport::default_plane(kind)
            };
            view.plan_frame = frame;
            (
                NaVector3::from(frame.x_axis().as_vector().to_array()),
                NaVector3::from(frame.y_axis().as_vector().to_array()),
                -NaVector3::from(frame.z_axis().as_vector().to_array()),
            )
        };
        let camera = view.target - forward * view.perspective_camera_distance;
        let points = [(1., 0.), (16., 0.), (8., 2.), (8., 3.)].map(|(depth, offset)| {
            let scale = if kind.is_parallel() {
                1. / view.pixels_per_unit
            } else {
                depth / view.perspective_focal_length_pixels(rect)
            };
            let p = camera + forward * depth + right * (offset * scale) + up * 0.;
            Point3::try_new(p.x, p.y, p.z).unwrap()
        });
        let cloud = PointCloud3::try_new(points.to_vec()).unwrap();
        let hit = view
            .pick_point_cloud_member(rect.center(), rect, &cloud)
            .unwrap();
        assert_eq!(hit.0, 2, "{kind:?}");
        assert!((hit.1 - 2.).abs() < 1e-4);
        assert_eq!(
            view.pick_point_cloud_member(
                rect.center(),
                rect,
                &cloud.with_hidden(vec![false, false, true, false]).unwrap()
            )
            .unwrap()
            .0,
            3
        );
        assert_eq!(
            view.point_cloud_members_in_window(&cloud, rect, rect),
            vec![2, 3]
        );
    }
}

#[test]
fn clipped_face_picking_matches_independent_rays_and_retains_face_indices() {
    use super::raster_tests::ray_triangle;
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.));
    for kind in [
        ViewKind::Top,
        ViewKind::Bottom,
        ViewKind::Front,
        ViewKind::Back,
        ViewKind::Right,
        ViewKind::Left,
        ViewKind::Plan,
        ViewKind::Perspective,
    ] {
        let mut view = Viewport::new(kind);
        view.frustum_near = 4.;
        view.frustum_far = 12.;
        if kind == ViewKind::Plan {
            view.plan_frame = Frame3::try_from_directions(
                Point3::try_new(0., 0., 0.).unwrap(),
                Vector3::try_new(1., 1., 0.).unwrap(),
                Vector3::try_new(-1., 1., 1.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap();
        }
        let (right, up, forward) = if kind == ViewKind::Perspective {
            view.perspective_basis()
        } else {
            let frame = if kind == ViewKind::Plan {
                view.plan_frame
            } else {
                Viewport::default_plane(kind)
            };
            (
                NaVector3::from(frame.x_axis().as_vector().to_array()),
                NaVector3::from(frame.y_axis().as_vector().to_array()),
                -NaVector3::from(frame.z_axis().as_vector().to_array()),
            )
        };
        let camera = view.target - forward * view.perspective_camera_distance;
        let points = [(-4., -3., 1.), (4., -3., 8.), (0., 4., 16.)].map(|(x, y, depth)| {
            let p = camera + right * x + up * y + forward * depth;
            Point3::try_new(p.x, p.y, p.z).unwrap()
        });
        for order in [[0, 1, 2], [2, 1, 0]] {
            let points = order.map(|i| points[i]);
            let mesh = TriangleMesh::try_new(points.to_vec(), vec![[0, 1, 2]], Tolerance::DEFAULT)
                .unwrap();
            let mut document = Document::default();
            let id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
            for mode in [DisplayMode::Shaded, DisplayMode::Ghosted] {
                view.display_mode = mode;
                let mut visible = 0;
                let mut clipped = 0;
                for y in (0..256).step_by(8) {
                    for x in (0..256).step_by(8) {
                        let pointer = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                        let dx = f64::from(pointer.x) - 128.;
                        let dy = 128. - f64::from(pointer.y);
                        let (origin, direction) = if kind.is_parallel() {
                            (
                                camera + (right * dx + up * dy) / view.pixels_per_unit,
                                forward,
                            )
                        } else {
                            (
                                camera,
                                forward
                                    + (right * dx + up * dy)
                                        / view.perspective_focal_length_pixels(rect),
                            )
                        };
                        let Some([depth, a, b, c]) = ray_triangle(origin, direction, points) else {
                            continue;
                        };
                        if a.min(b).min(c).abs() < 0.02
                            || (depth - 4.).abs() < 0.02
                            || (depth - 12.).abs() < 0.02
                        {
                            continue;
                        }
                        let in_face = depth > 0. && a.min(b).min(c) > 0.;
                        let expected = in_face && (4. ..=12.).contains(&depth);
                        let hit = view.mesh_face_pick(pointer, rect, &mesh);
                        assert_eq!(
                            hit.is_some_and(|(h, _)| h.distance == 0.),
                            expected,
                            "{kind:?} {mode:?} {x},{y} depth={depth}"
                        );
                        if expected {
                            assert_eq!(hit.unwrap().1, 0);
                            visible += 1;
                        } else if in_face {
                            clipped += 1;
                        }
                    }
                }
                assert!(visible > 0 && clipped > 0);
                assert_eq!(
                    view.objects_in_selection(rect, rect, false, &document),
                    vec![]
                );
                assert_eq!(
                    view.objects_in_selection(rect, rect, true, &document),
                    vec![id]
                );
            }
        }
    }
}
