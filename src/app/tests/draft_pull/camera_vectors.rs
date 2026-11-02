//! Nonunit camera directions survive app files and drive the native draft View choice.
use super::*;
use serde_json::Value;
use viboceros_command::construction_plane::WorldPlane;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn vector(value: &Value) -> Vector3 {
    Vector3::try_from(serde_json::from_value::<[f64; 3]>(value.clone()).unwrap()).unwrap()
}
fn exported(app: &VibocerosApp) -> ThreeDmNamedView {
    Viewport::named_view_to_3dm(app.viewports[0].named_view_snapshot(), "Raw".into()).unwrap()
}

#[test]
fn camera_vectors_survive_app_files_named_views_and_draft_view_controls() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/rhino_oracle");
    let q: Value =
        serde_json::from_slice(&std::fs::read(root.join("fixtures/camera_vectors.json")).unwrap())
            .unwrap();
    let n: Value = serde_json::from_slice(
        &std::fs::read(root.join("observations/camera_vectors.json")).unwrap(),
    )
    .unwrap();
    let path =
        std::env::temp_dir().join(format!("viboceros-raw-camera-{}.3dm", std::process::id()));
    let (mut app, id) = setup();
    let geometry = app.document.object(id).unwrap().geometry_snapshot().clone();
    let mut count = 0;
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(n["results"].as_array().unwrap())
    {
        if op["op"] != "draft_pull_direction" {
            continue;
        }
        assert_eq!(op["id"], row["id"]);
        let direction = vector(&op["view"]);
        let frame =
            Frame3::try_from_normal(point(0., 0., 0.), direction, Tolerance::DEFAULT).unwrap();
        for projection in [
            ThreeDmProjection::Parallel,
            ThreeDmProjection::Perspective,
            ThreeDmProjection::TwoPointPerspective,
        ] {
            let camera = ThreeDmNamedView {
                camera_axes: None,
                name: "Raw".into(),
                projection,
                camera_location: point(10., 20., 30.),
                camera_direction: direction,
                camera_up: frame.y_axis().as_vector().scaled(7.).unwrap(),
                target: Some(point(0., 0., 0.)),
                construction_plane: WorldPlane::Top.frame(),
                frustum: [-2., 2., -1.5, 1.5, 1., 1000.],
                screen_port: [0, 800, 600, 0],
            };
            app.viewports[0].restore_file_camera(&camera).unwrap();
            // Save/update and restore through real app commands, while a model
            // getter and its redo branch remain live.
            enter(&mut app, "Point 5,5,5");
            enter(&mut app, "Undo");
            enter(&mut app, "Line");
            let active = app.active_command;
            if app.named_views.get("Raw").is_ok() {
                enter(&mut app, "NamedView Update Raw");
            } else {
                enter(&mut app, "NamedView Save Raw");
            }
            enter(&mut app, "SetView World Top");
            enter(&mut app, "NamedView Restore Raw");
            enter(&mut app, "DraftAngleAnalysis Direction=View");
            assert_eq!(app.active_command, active);
            assert!(app.document.can_redo());
            assert_eq!(
                app.document.object(id).unwrap().geometry_snapshot(),
                &geometry
            );
            let expected: [f64; 3] = serde_json::from_value(row["value"]["up"].clone()).unwrap();
            assert_eq!(up(&app), expected);
            let current = exported(&app);
            assert_eq!(current.camera_direction, camera.camera_direction);
            assert_eq!(current.camera_up, camera.camera_up);
            app.cancel_interactive_command(false);
            enter(&mut app, &format!("SaveAs \"{}\"", path.display()));
            assert!(
                app.command_log.back().unwrap().starts_with("Saved"),
                "{:?}",
                app.command_log.back()
            );
            let model = viboceros_io::read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
            assert_eq!(model.named_views[0].camera_direction, direction);
            assert_eq!(model.named_views[0].camera_up, camera.camera_up);
            assert_eq!(model.viewports[0].camera.camera_direction, direction);
            let mut opened = test_app();
            enter(&mut opened, &format!("Open \"{}\"", path.display()));
            assert_eq!(exported(&opened).camera_direction, direction);
            assert_eq!(exported(&opened).camera_up, camera.camera_up);
            count += 1;
        }
    }
    std::fs::remove_file(path).unwrap();
    assert_eq!(count, 12);
}
