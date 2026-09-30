use super::*;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

pub(super) fn captures() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../tools/rhino_oracle/observations/viewport_clip_transform.json"
    ))
    .unwrap()
}

pub(super) fn captured_view(camera: &serde_json::Value) -> Viewport {
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
